import com.google.gson.*;
import java.lang.reflect.Method;
import java.nio.file.*;
import java.util.*;
import net.minecraft.core.Holder;
import net.minecraft.tags.EnchantmentTags;
import net.minecraft.world.SimpleContainer;
import net.minecraft.world.inventory.*;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.enchantment.*;

/**
 * Drives the real Java 26.2 {@code GrindstoneMenu.computeResult} over a case
 * matrix and records the disenchanted/merged result plus the deterministic
 * experience contribution of each input.
 *
 * The random half of {@code getExperienceAmount} is excluded: it draws from the
 * level's RNG, so only the pre-random sum is comparable. That sum is computed
 * with the genuine {@code getMinCost} and curse-tag lookups.
 */
public class GrindstoneOracle extends WorkstationSupport {
    /** Suppresses the listener broadcast; the probe has no slots or listeners. */
    public static class Probe extends GrindstoneMenu {
        private Probe() {
            super(0, null);
        }

        @Override
        public void broadcastChanges() {}
    }

    /** Mirrors the anonymous result slot's private getExperienceFromItem. */
    static int experienceFromItem(ItemStack item) {
        int amount = 0;
        for (var entry : EnchantmentHelper.getEnchantmentsForCrafting(item).entrySet()) {
            Holder<Enchantment> enchant = entry.getKey();
            if (!enchant.is(EnchantmentTags.CURSE)) amount += enchant.value().getMinCost(entry.getIntValue());
        }
        return amount;
    }

    public static void main(String[] args) throws Exception {
        bootstrap();

        Probe menu = allocate(Probe.class);
        SimpleContainer repair = new SimpleContainer(2);
        ResultContainer result = new ResultContainer();
        set(GrindstoneMenu.class, menu, "repairSlots", repair);
        set(GrindstoneMenu.class, menu, "resultSlots", result);

        Method compute = GrindstoneMenu.class.getDeclaredMethod("computeResult", ItemStack.class, ItemStack.class);
        compute.setAccessible(true);

        JsonArray cases = new JsonArray();
        for (Spec[] pair : scenarios()) {
            ItemStack input = build(pair[0]);
            ItemStack additional = build(pair[1]);
            ItemStack computed = (ItemStack) compute.invoke(menu, input, additional);

            JsonObject entry = new JsonObject();
            entry.add("input", spec(pair[0]));
            entry.add("additional", spec(pair[1]));
            entry.add("result", describe(computed));
            entry.addProperty("experience_input", experienceFromItem(input));
            entry.addProperty("experience_additional", experienceFromItem(additional));
            cases.add(entry);
        }
        Files.writeString(Path.of(args[0]), GSON.toJson(cases));
        System.err.println("grindstone cases: " + cases.size());
    }

    static List<Spec[]> scenarios() {
        List<Spec[]> out = new ArrayList<>();
        String[] tools = {"minecraft:diamond_sword", "minecraft:iron_pickaxe", "minecraft:netherite_axe",
                "minecraft:diamond_chestplate", "minecraft:bow", "minecraft:elytra", "minecraft:shears",
                "minecraft:turtle_helmet", "minecraft:fishing_rod", "minecraft:trident"};
        List<Map<String, Integer>> spells = List.of(
                Map.of(),
                Map.of("minecraft:sharpness", 3),
                Map.of("minecraft:unbreaking", 3),
                Map.of("minecraft:mending", 1),
                Map.of("minecraft:binding_curse", 1),
                Map.of("minecraft:vanishing_curse", 1),
                Map.of("minecraft:binding_curse", 1, "minecraft:vanishing_curse", 1),
                Map.of("minecraft:sharpness", 5, "minecraft:vanishing_curse", 1),
                Map.of("minecraft:efficiency", 5, "minecraft:fortune", 3, "minecraft:mending", 1),
                Map.of("minecraft:protection", 4, "minecraft:binding_curse", 1));

        // Disenchant a single item, including curse-only and mixed-curse stacks.
        for (String tool : tools)
            for (Map<String, Integer> spell : spells) {
                int max = maxDamage(tool);
                for (int damage : new int[] {0, max / 2, Math.max(0, max - 1)})
                    out.add(new Spec[] {new Spec(tool, 1, damage, 0, spell, Map.of(), null), null});
            }

        // Merge two of the same tool: durability sum plus the 5% bonus.
        for (String tool : tools) {
            int max = maxDamage(tool);
            for (int left : new int[] {0, max / 4, max / 2, Math.max(0, max - 1)})
                for (int right : new int[] {0, max / 2, Math.max(0, max - 1)})
                    for (Map<String, Integer> spell : List.of(Map.<String, Integer>of(),
                            Map.of("minecraft:sharpness", 3), Map.of("minecraft:vanishing_curse", 1)))
                        out.add(new Spec[] {new Spec(tool, 1, left, 0, spell, Map.of(), null),
                                new Spec(tool, 1, right, 0, spell, Map.of(), null)});
        }

        // Mismatched pairs, books and non-enchantables.
        out.add(new Spec[] {null, null});
        out.add(new Spec[] {Spec.of("minecraft:diamond_sword"), null});
        out.add(new Spec[] {null, Spec.of("minecraft:diamond_sword")});
        out.add(new Spec[] {Spec.of("minecraft:diamond_sword"), Spec.of("minecraft:iron_pickaxe")});
        out.add(new Spec[] {Spec.of("minecraft:stone"), Spec.of("minecraft:stone")});
        out.add(new Spec[] {Spec.book(Map.of("minecraft:sharpness", 3)), null});
        out.add(new Spec[] {Spec.book(Map.of("minecraft:sharpness", 3)),
                Spec.book(Map.of("minecraft:sharpness", 3))});
        out.add(new Spec[] {Spec.enchanted("minecraft:diamond_sword", Map.of("minecraft:sharpness", 3)),
                Spec.book(Map.of("minecraft:smite", 2))});
        // Stacked inputs: the grindstone only ever consumes single items.
        out.add(new Spec[] {new Spec("minecraft:diamond_sword", 2, 0, 0,
                Map.of("minecraft:sharpness", 1), Map.of(), null), null});
        return out;
    }
}
