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
import net.minecraft.world.item.*;
import net.minecraft.world.item.enchantment.*;
import sun.misc.Unsafe;

/**
 * Shared headless bootstrap and encoding for the workstation parity probes.
 *
 * Boots the vanilla registries without a server, loads the enchantment registry
 * from the shipped datapack, binds every item's real component map and exposes
 * the stack encoding the Rust replay tests expect.
 *
 * {@code AnvilOracle} predates this class and carries its own copy.
 */
public class WorkstationSupport {
    public static final Gson GSON = new GsonBuilder().serializeNulls().create();
    public static final Path DATA = Path.of("assets/datapacks/26_2/data/minecraft");
    public static Unsafe unsafe;
    public static Registry<Enchantment> enchantments;
    public static Registry<net.minecraft.world.level.block.entity.BannerPattern> bannerPatterns;
    public static RegistryOps<JsonElement> ops;

    protected WorkstationSupport() {}

    /** Reflectively writes a field, including a final one, on a probe instance. */
    public static void set(Class<?> owner, Object target, String name, Object value) throws Exception {
        var field = owner.getDeclaredField(name);
        field.setAccessible(true);
        field.set(target, value);
    }

    public static Object get(Class<?> owner, Object target, String name) throws Exception {
        var field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(target);
    }

    @SuppressWarnings("unchecked")
    public static <T> T allocate(Class<T> type) throws Exception {
        return (T) unsafe.allocateInstance(type);
    }

    public static <T> List<Holder<T>> tag(String folder, String name, Registry<T> registry) throws Exception {
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

    public static <T> Map<TagKey<T>, List<Holder<T>>> tags(String folder, Registry<T> registry) throws Exception {
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

    /** Boots registries, enchantments and real item components. Call once. */
    public static void bootstrap() throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var field = Unsafe.class.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        unsafe = (Unsafe) field.get(null);

        // Item tags first: the enchantment codec references them.
        BuiltInRegistries.ITEM.prepareTagReload(
                new TagLoader.LoadResult<>(Registries.ITEM, tags("item", BuiltInRegistries.ITEM))).apply();

        var base = net.minecraft.data.registries.VanillaRegistries.createLookup();
        var registry = new MappedRegistry<Enchantment>(Registries.ENCHANTMENT, Lifecycle.stable());
        var pending = registry.createRegistrationLookup();
        var patterns = new MappedRegistry<net.minecraft.world.level.block.entity.BannerPattern>(
                Registries.BANNER_PATTERN, Lifecycle.stable());
        var pendingPatterns = patterns.createRegistrationLookup();
        ops = RegistryOps.create(JsonOps.INSTANCE, new RegistryOps.RegistryInfoLookup() {
            @SuppressWarnings({"unchecked", "rawtypes"})
            public <T> Optional<RegistryOps.RegistryInfo<T>> lookup(ResourceKey<? extends Registry<? extends T>> key) {
                if (key.equals(Registries.ENCHANTMENT))
                    return (Optional) Optional.of(new RegistryOps.RegistryInfo<>(registry, pending, Lifecycle.stable()));
                if (key.equals(Registries.BANNER_PATTERN))
                    return (Optional) Optional.of(
                            new RegistryOps.RegistryInfo<>(patterns, pendingPatterns, Lifecycle.stable()));
                if (key.equals(Registries.ITEM))
                    return (Optional) Optional.of(RegistryOps.RegistryInfo.fromRegistryLookup(BuiltInRegistries.ITEM));
                return base.lookup(key).map(RegistryOps.RegistryInfo::fromRegistryLookup);
            }
        });

        // Banner patterns must exist before item components, which reference
        // their tags through provides_banner_patterns.
        try (var files = Files.list(DATA.resolve("banner_pattern"))) {
            for (var path : files.sorted().toList())
                patterns.register(
                        ResourceKey.create(Registries.BANNER_PATTERN,
                                Identifier.withDefaultNamespace(path.getFileName().toString().replace(".json", ""))),
                        net.minecraft.world.level.block.entity.BannerPattern.DIRECT_CODEC
                                .parse(ops, JsonParser.parseString(Files.readString(path))).getOrThrow(),
                        RegistrationInfo.BUILT_IN);
        }
        patterns.bindTags(tags("banner_pattern", patterns));
        patterns.freeze();
        bannerPatterns = patterns;
        try (var files = Files.list(DATA.resolve("enchantment"))) {
            for (var path : files.sorted().toList())
                registry.register(
                        ResourceKey.create(Registries.ENCHANTMENT,
                                Identifier.withDefaultNamespace(path.getFileName().toString().replace(".json", ""))),
                        Enchantment.DIRECT_CODEC.parse(ops, JsonParser.parseString(Files.readString(path))).getOrThrow(),
                        RegistrationInfo.BUILT_IN);
        }
        registry.bindTags(tags("enchantment", registry));
        registry.freeze();
        enchantments = registry;

        var items = JsonParser.parseString(Files.readString(Path.of("assets/items.json"))).getAsJsonObject();
        for (Item item : BuiltInRegistries.ITEM) {
            var source = items.getAsJsonObject(BuiltInRegistries.ITEM.getKey(item).getPath())
                    .getAsJsonObject("components");
            item.builtInRegistryHolder().bindComponents(DataComponentMap.CODEC.parse(ops, source).getOrThrow());
        }
    }

    public static int maxDamage(String id) {
        return BuiltInRegistries.ITEM.get(Identifier.parse(id)).orElseThrow().value()
                .components().getOrDefault(DataComponents.MAX_DAMAGE, 0);
    }

    // ── stack specs ─────────────────────────────────────────────────────────
    public record Spec(String item, int count, int damage, int repairCost, Map<String, Integer> enchants,
                       Map<String, Integer> stored, String customName) {
        public static Spec of(String item) {
            return new Spec(item, 1, 0, 0, Map.of(), Map.of(), null);
        }

        public static Spec damaged(String item, int damage) {
            return new Spec(item, 1, damage, 0, Map.of(), Map.of(), null);
        }

        public static Spec enchanted(String item, Map<String, Integer> enchants) {
            return new Spec(item, 1, 0, 0, enchants, Map.of(), null);
        }

        public static Spec book(Map<String, Integer> stored) {
            return new Spec("minecraft:enchanted_book", 1, 0, 0, Map.of(), stored, null);
        }
    }

    public static ItemStack build(Spec spec) {
        if (spec == null) return ItemStack.EMPTY;
        ItemStack stack = new ItemStack(
                BuiltInRegistries.ITEM.get(Identifier.parse(spec.item())).orElseThrow(), spec.count());
        if (spec.damage() != 0) stack.set(DataComponents.DAMAGE, spec.damage());
        if (spec.repairCost() != 0) stack.set(DataComponents.REPAIR_COST, spec.repairCost());
        if (!spec.enchants().isEmpty()) stack.set(DataComponents.ENCHANTMENTS, enchantmentsOf(spec.enchants()));
        if (!spec.stored().isEmpty())
            stack.set(DataComponents.STORED_ENCHANTMENTS, enchantmentsOf(spec.stored()));
        if (spec.customName() != null)
            stack.set(DataComponents.CUSTOM_NAME, net.minecraft.network.chat.Component.literal(spec.customName()));
        return stack;
    }

    static ItemEnchantments enchantmentsOf(Map<String, Integer> values) {
        var mutable = new ItemEnchantments.Mutable(ItemEnchantments.EMPTY);
        values.forEach((id, level) -> mutable.set(enchantments.get(Identifier.parse(id)).orElseThrow(), level));
        return mutable.toImmutable();
    }

    public static JsonElement spec(Spec spec) {
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

    public static JsonElement describe(ItemStack stack) {
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
        var trim = stack.get(DataComponents.TRIM);
        if (trim == null) {
            out.add("trim", JsonNull.INSTANCE);
        } else {
            JsonObject encoded = new JsonObject();
            encoded.addProperty("material", trim.material().unwrapKey().orElseThrow().identifier().toString());
            encoded.addProperty("pattern", trim.pattern().unwrapKey().orElseThrow().identifier().toString());
            out.add("trim", encoded);
        }
        return out;
    }

    public static JsonObject encode(ItemEnchantments values) {
        JsonObject out = new JsonObject();
        var entries = new TreeMap<String, Integer>();
        for (var holder : values.keySet())
            entries.put(holder.unwrapKey().orElseThrow().identifier().toString(), values.getLevel(holder));
        entries.forEach(out::addProperty);
        return out;
    }
}
