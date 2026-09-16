import com.google.gson.*;
import com.mojang.serialization.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.core.*;
import net.minecraft.core.component.*;
import net.minecraft.core.registries.*;
import net.minecraft.resources.*;
import net.minecraft.server.Bootstrap;
import net.minecraft.tags.*;
import net.minecraft.world.SimpleContainer;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.*;
import net.minecraft.world.item.*;
import net.minecraft.world.item.enchantment.*;
import net.minecraft.world.level.GameType;
import sun.misc.Unsafe;

/**
 * Drives the real Java 26.2 {@link AnvilMenu#createResult()} over a fixed case
 * matrix and records the level cost, consumed material count and the complete
 * result stack. The Rust side replays the recorded inputs and compares.
 */
public class AnvilOracle {
    static final Gson GSON = new GsonBuilder().serializeNulls().create();
    static final Path DATA = Path.of("assets/datapacks/26_2/data/minecraft");
    static Unsafe unsafe;

    /** Only {@code hasInfiniteMaterials} is reached from {@code createResult}. */
    public static class FakePlayer extends Player {
        public boolean creative;

        private FakePlayer() {
            super(null, null);
        }

        @Override
        public GameType gameMode() {
            return this.creative ? GameType.CREATIVE : GameType.SURVIVAL;
        }

        @Override
        public boolean hasInfiniteMaterials() {
            return this.creative;
        }
    }

    /** Suppresses the listener broadcast; the probe has no slots or listeners. */
    public static class Probe extends AnvilMenu {
        private Probe() {
            super(0, null);
        }

        @Override
        public void broadcastChanges() {}
    }

    static void set(Class<?> owner, Object target, String name, Object value) throws Exception {
        var field = owner.getDeclaredField(name);
        field.setAccessible(true);
        field.set(target, value);
    }

    static <T> List<Holder<T>> tag(String folder, String name, Registry<T> registry) throws Exception {
        var result = new LinkedHashSet<Holder<T>>();
        var data = JsonParser.parseString(Files.readString(DATA.resolve("tags/" + folder + "/" + name + ".json")))
                .getAsJsonObject();
        for (var entry : data.getAsJsonArray("values")) {
            String id = entry.isJsonObject() ? entry.getAsJsonObject().get("id").getAsString() : entry.getAsString();
            if (id.startsWith("#")) result.addAll(tag(folder, id.substring(1).replace("minecraft:", ""), registry));
            else result.add(registry.get(Identifier.parse(id)).orElseThrow());
        }
        return List.copyOf(result);
    }

    static <T> Map<TagKey<T>, List<Holder<T>>> tags(String folder, Registry<T> registry) throws Exception {
        Map<TagKey<T>, List<Holder<T>>> result = new HashMap<>();
        Path root = DATA.resolve("tags/" + folder);
        try (var files = Files.walk(root)) {
            for (var path : files.filter(p -> p.toString().endsWith(".json")).sorted().toList()) {
                String name = root.relativize(path).toString().replace(".json", "");
                result.put(TagKey.create(registry.key(), Identifier.withDefaultNamespace(name)),
                        tag(folder, name, registry));
            }
        }
        return result;
    }

    // ── case description ────────────────────────────────────────────────────
    record Spec(String item, int count, int damage, int repairCost, Map<String, Integer> enchants,
                Map<String, Integer> stored, String customName) {}

    static ItemStack build(Spec spec, Registry<Enchantment> enchantments) {
        if (spec == null) return ItemStack.EMPTY;
        ItemStack stack = new ItemStack(BuiltInRegistries.ITEM.get(Identifier.parse(spec.item())).orElseThrow(),
                spec.count());
        if (spec.damage() != 0) stack.set(DataComponents.DAMAGE, spec.damage());
        if (spec.repairCost() != 0) stack.set(DataComponents.REPAIR_COST, spec.repairCost());
        if (!spec.enchants().isEmpty()) {
            var mutable = new ItemEnchantments.Mutable(ItemEnchantments.EMPTY);
            spec.enchants().forEach((id, level) ->
                    mutable.set(enchantments.get(Identifier.parse(id)).orElseThrow(), level));
            stack.set(DataComponents.ENCHANTMENTS, mutable.toImmutable());
        }
        if (!spec.stored().isEmpty()) {
            var mutable = new ItemEnchantments.Mutable(ItemEnchantments.EMPTY);
            spec.stored().forEach((id, level) ->
                    mutable.set(enchantments.get(Identifier.parse(id)).orElseThrow(), level));
            stack.set(DataComponents.STORED_ENCHANTMENTS, mutable.toImmutable());
        }
        if (spec.customName() != null)
            stack.set(DataComponents.CUSTOM_NAME, net.minecraft.network.chat.Component.literal(spec.customName()));
        return stack;
    }

    static JsonElement describe(ItemStack stack) {
        if (stack.isEmpty()) return JsonNull.INSTANCE;
        JsonObject out = new JsonObject();
        out.addProperty("item", BuiltInRegistries.ITEM.getKey(stack.getItem()).toString());
        out.addProperty("count", stack.getCount());
        out.addProperty("damage", stack.getOrDefault(DataComponents.DAMAGE, 0));
        out.addProperty("repair_cost", stack.getOrDefault(DataComponents.REPAIR_COST, 0));
        var name = stack.get(DataComponents.CUSTOM_NAME);
        out.add("custom_name", name == null ? JsonNull.INSTANCE : new JsonPrimitive(name.getString()));
        out.add("enchantments", encode(stack.getOrDefault(DataComponents.ENCHANTMENTS, ItemEnchantments.EMPTY)));
        out.add("stored_enchantments",
                encode(stack.getOrDefault(DataComponents.STORED_ENCHANTMENTS, ItemEnchantments.EMPTY)));
        return out;
    }

    static JsonObject encode(ItemEnchantments enchantments) {
        JsonObject out = new JsonObject();
        var entries = new TreeMap<String, Integer>();
        for (var holder : enchantments.keySet())
            entries.put(holder.unwrapKey().orElseThrow().identifier().toString(), enchantments.getLevel(holder));
        entries.forEach(out::addProperty);
        return out;
    }

    static JsonElement spec(Spec spec) {
        if (spec == null) return JsonNull.INSTANCE;
        JsonObject out = new JsonObject();
        out.addProperty("item", spec.item());
        out.addProperty("count", spec.count());
        out.addProperty("damage", spec.damage());
        out.addProperty("repair_cost", spec.repairCost());
        out.add("custom_name", spec.customName() == null ? JsonNull.INSTANCE : new JsonPrimitive(spec.customName()));
        out.add("enchantments", GSON.toJsonTree(new TreeMap<>(spec.enchants())));
        out.add("stored_enchantments", GSON.toJsonTree(new TreeMap<>(spec.stored())));
        return out;
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var field = Unsafe.class.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        unsafe = (Unsafe) field.get(null);

        // Item tags first: the enchantment codec references them.
        BuiltInRegistries.ITEM.prepareTagReload(
                new TagLoader.LoadResult<>(Registries.ITEM, tags("item", BuiltInRegistries.ITEM))).apply();

        var base = net.minecraft.data.registries.VanillaRegistries.createLookup();
        var enchantments = new MappedRegistry<Enchantment>(Registries.ENCHANTMENT, Lifecycle.stable());
        var pending = enchantments.createRegistrationLookup();
        var ops = RegistryOps.create(JsonOps.INSTANCE, new RegistryOps.RegistryInfoLookup() {
            @SuppressWarnings({"unchecked", "rawtypes"})
            public <T> Optional<RegistryOps.RegistryInfo<T>> lookup(ResourceKey<? extends Registry<? extends T>> key) {
                if (key.equals(Registries.ENCHANTMENT))
                    return (Optional) Optional.of(
                            new RegistryOps.RegistryInfo<>(enchantments, pending, Lifecycle.stable()));
                if (key.equals(Registries.ITEM))
                    return (Optional) Optional.of(RegistryOps.RegistryInfo.fromRegistryLookup(BuiltInRegistries.ITEM));
                return base.lookup(key).map(RegistryOps.RegistryInfo::fromRegistryLookup);
            }
        });
        try (var files = Files.list(DATA.resolve("enchantment"))) {
            for (var path : files.sorted().toList())
                enchantments.register(
                        ResourceKey.create(Registries.ENCHANTMENT,
                                Identifier.withDefaultNamespace(path.getFileName().toString().replace(".json", ""))),
                        Enchantment.DIRECT_CODEC.parse(ops, JsonParser.parseString(Files.readString(path)))
                                .getOrThrow(),
                        RegistrationInfo.BUILT_IN);
        }
        enchantments.bindTags(tags("enchantment", enchantments));
        enchantments.freeze();

        // Real item components: damage, max_damage, repairable, enchantability.
        var items = JsonParser.parseString(Files.readString(Path.of("assets/items.json"))).getAsJsonObject();
        for (Item item : BuiltInRegistries.ITEM) {
            var source = items.getAsJsonObject(BuiltInRegistries.ITEM.getKey(item).getPath())
                    .getAsJsonObject("components");
            item.builtInRegistryHolder()
                    .bindComponents(DataComponentMap.CODEC.parse(ops, source).getOrThrow());
        }

        FakePlayer player = (FakePlayer) unsafe.allocateInstance(FakePlayer.class);
        Probe menu = (Probe) unsafe.allocateInstance(Probe.class);
        SimpleContainer inputs = new SimpleContainer(2);
        set(ItemCombinerMenu.class, menu, "inputSlots", inputs);
        set(ItemCombinerMenu.class, menu, "resultSlots", new ResultContainer());
        set(ItemCombinerMenu.class, menu, "player", player);
        set(AnvilMenu.class, menu, "cost", DataSlot.standalone());

        var costField = AnvilMenu.class.getDeclaredField("cost");
        costField.setAccessible(true);
        var resultField = ItemCombinerMenu.class.getDeclaredField("resultSlots");
        resultField.setAccessible(true);
        var repairCountField = AnvilMenu.class.getDeclaredField("repairItemCountCost");
        repairCountField.setAccessible(true);

        JsonArray cases = new JsonArray();
        java.util.function.ToIntFunction<String> maxDamage = id -> BuiltInRegistries.ITEM
                .get(Identifier.parse(id)).orElseThrow().value()
                .components().getOrDefault(DataComponents.MAX_DAMAGE, 0);
        for (Object[] scenario : scenarios(maxDamage)) {
            Spec left = (Spec) scenario[0];
            Spec right = (Spec) scenario[1];
            String name = (String) scenario[2];
            boolean creative = (Boolean) scenario[3];

            player.creative = creative;
            inputs.setItem(0, build(left, enchantments));
            inputs.setItem(1, build(right, enchantments));
            set(AnvilMenu.class, menu, "itemName", name);
            set(AnvilMenu.class, menu, "repairItemCountCost", 0);
            set(AnvilMenu.class, menu, "onlyRenaming", false);
            ((DataSlot) costField.get(menu)).set(0);
            ((ResultContainer) resultField.get(menu)).setItem(0, ItemStack.EMPTY);

            menu.createResult();

            JsonObject entry = new JsonObject();
            entry.add("left", spec(left));
            entry.add("right", spec(right));
            entry.add("name", name == null ? JsonNull.INSTANCE : new JsonPrimitive(name));
            entry.addProperty("creative", creative);
            entry.addProperty("cost", ((DataSlot) costField.get(menu)).get());
            entry.addProperty("material_cost", repairCountField.getInt(menu));
            entry.add("result", describe(((ResultContainer) resultField.get(menu)).getItem(0)));
            cases.add(entry);
        }
        Files.writeString(Path.of(args[0]), GSON.toJson(cases));
        System.err.println("anvil cases: " + cases.size());
    }

    static Spec plain(String item) {
        return new Spec(item, 1, 0, 0, Map.of(), Map.of(), null);
    }

    static Spec damaged(String item, int damage, int repairCost) {
        return new Spec(item, 1, damage, repairCost, Map.of(), Map.of(), null);
    }

    static Spec enchanted(String item, Map<String, Integer> enchants, int damage, int repairCost) {
        return new Spec(item, 1, damage, repairCost, enchants, Map.of(), null);
    }

    static Spec book(Map<String, Integer> stored, int repairCost) {
        return new Spec("minecraft:enchanted_book", 1, 0, repairCost, Map.of(), stored, null);
    }

    /** {left, right, name, creative} covering each distinct branch of createResult. */
    static List<Object[]> scenarios(java.util.function.ToIntFunction<String> maxDamage) {
        List<Object[]> out = new ArrayList<>();
        String[] tools = {"minecraft:diamond_sword", "minecraft:iron_pickaxe", "minecraft:netherite_axe",
                "minecraft:diamond_chestplate", "minecraft:bow", "minecraft:elytra", "minecraft:shears",
                "minecraft:turtle_helmet"};
        String[] materials = {"minecraft:diamond", "minecraft:iron_ingot", "minecraft:netherite_ingot",
                "minecraft:phantom_membrane", "minecraft:turtle_scute", "minecraft:string", "minecraft:stick"};
        // Fractions of each item's own durability; Java clamps damage to
        // [0, maxDamage], so absolute values would test an unreachable state.
        double[] wear = {0.0, 0.25, 0.5, 0.75, 1.0};
        int[] priorWork = {0, 1, 3, 7, 15, 31};
        String[] names = {null, "", "Renamed", "x".repeat(50), "x".repeat(51)};

        // Repair with material, across damage, prior work and material identity.
        for (String tool : tools)
            for (double fraction : wear)
                for (String material : materials)
                    for (int work : new int[] {0, 3}) {
                        int damage = (int) (maxDamage.applyAsInt(tool) * fraction);
                        out.add(new Object[] {damaged(tool, damage, work),
                                new Spec(material, 1, 0, 0, Map.of(), Map.of(), null), null, false});
                        out.add(new Object[] {damaged(tool, damage, work),
                                new Spec(material, 4, 0, 0, Map.of(), Map.of(), null), null, false});
                    }

        // Combine two of the same tool, mixing damage, enchantments and prior work.
        for (String tool : tools)
            for (double fraction : wear)
                for (int work : priorWork) {
                    int damage = (int) (maxDamage.applyAsInt(tool) * fraction);
                    out.add(new Object[] {damaged(tool, damage, work), damaged(tool, damage / 2, work), null, false});
                    out.add(new Object[] {enchanted(tool, Map.of("minecraft:unbreaking", 2), damage, work),
                            enchanted(tool, Map.of("minecraft:unbreaking", 2), 0, 0), null, false});
                    out.add(new Object[] {enchanted(tool, Map.of("minecraft:unbreaking", 3), damage, work),
                            enchanted(tool, Map.of("minecraft:unbreaking", 1), 0, work), null, false});
                }

        // Books: compatible, incompatible, mutually exclusive, over-max level.
        List<Map<String, Integer>> spells = List.of(
                Map.of("minecraft:sharpness", 1), Map.of("minecraft:sharpness", 5),
                Map.of("minecraft:sharpness", 10), Map.of("minecraft:smite", 3),
                Map.of("minecraft:mending", 1), Map.of("minecraft:unbreaking", 3),
                Map.of("minecraft:efficiency", 4), Map.of("minecraft:silk_touch", 1),
                Map.of("minecraft:fortune", 3), Map.of("minecraft:sharpness", 2, "minecraft:unbreaking", 2),
                Map.of("minecraft:protection", 4, "minecraft:mending", 1));
        for (String tool : tools)
            for (Map<String, Integer> spell : spells)
                for (int work : new int[] {0, 7}) {
                    out.add(new Object[] {damaged(tool, 100, work), book(spell, 0), null, false});
                    out.add(new Object[] {enchanted(tool, Map.of("minecraft:sharpness", 3), 100, work),
                            book(spell, work), null, false});
                }

        // Renaming, including the length limit and a no-op rename.
        for (String tool : tools)
            for (String name : names)
                for (int work : new int[] {0, 31}) {
                    out.add(new Object[] {damaged(tool, 50, work), null, name, false});
                    out.add(new Object[] {damaged(tool, 50, work),
                            new Spec("minecraft:diamond", 1, 0, 0, Map.of(), Map.of(), null), name, false});
                }

        // Too-expensive boundary, in survival and creative.
        for (int work : new int[] {0, 15, 31, 63, 255, 1023, Integer.MAX_VALUE / 2})
            for (boolean creative : new boolean[] {false, true}) {
                out.add(new Object[] {enchanted("minecraft:diamond_sword",
                        Map.of("minecraft:sharpness", 5, "minecraft:unbreaking", 3), 100, work),
                        enchanted("minecraft:diamond_sword",
                                Map.of("minecraft:sharpness", 5, "minecraft:looting", 3), 0, work),
                        "Costly", creative});
                out.add(new Object[] {enchanted("minecraft:diamond_sword", Map.of("minecraft:sharpness", 5), 0, work),
                        book(Map.of("minecraft:sharpness", 5), work), null, creative});
            }

        // Durability boundaries: undamaged, one point of wear, and fully spent.
        for (String tool : tools) {
            int max = maxDamage.applyAsInt(tool);
            for (int damage : new int[] {0, 1, max - 1, max})
                out.add(new Object[] {damaged(tool, damage, 0),
                        new Spec("minecraft:diamond", 1, 0, 0, Map.of(), Map.of(), null), null, false});
        }

        // Degenerate and mismatched inputs.
        out.add(new Object[] {null, null, null, false});
        out.add(new Object[] {null, plain("minecraft:diamond"), null, false});
        out.add(new Object[] {plain("minecraft:stone"), plain("minecraft:stone"), null, false});
        out.add(new Object[] {plain("minecraft:diamond_sword"), plain("minecraft:iron_pickaxe"), null, false});
        out.add(new Object[] {damaged("minecraft:diamond_sword", 0, 0), plain("minecraft:diamond"), null, false});
        out.add(new Object[] {book(Map.of("minecraft:sharpness", 3), 0), book(Map.of("minecraft:sharpness", 3), 0),
                null, false});
        out.add(new Object[] {plain("minecraft:enchanted_book"), plain("minecraft:diamond"), null, false});
        return out;
    }
}
