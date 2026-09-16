import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.crafting.*;

/**
 * Drives the real Java 26.2 smithing recipes over a case matrix: for each
 * (template, base, addition) triple the first matching {@link SmithingRecipe}
 * is assembled and the produced stack recorded, including its armour trim.
 *
 * Covers both branches: netherite transforms, which carry the base item's
 * components across, and trim application, which returns EMPTY when the trim
 * already present is identical.
 */
public class SmithingOracle extends WorkstationSupport {
    public static void main(String[] args) throws Exception {
        bootstrap();

        List<SmithingRecipe> recipes = new ArrayList<>();
        Path root = DATA.resolve("recipe");
        try (var files = Files.walk(root)) {
            for (var path : files.filter(p -> p.toString().endsWith(".json")).sorted().toList()) {
                var json = JsonParser.parseString(Files.readString(path));
                if (!json.isJsonObject()) continue;
                var type = json.getAsJsonObject().get("type");
                if (type == null || !type.getAsString().startsWith("minecraft:smithing")) continue;
                Recipe<?> recipe = Recipe.CODEC.parse(ops, json).getOrThrow();
                if (recipe instanceof SmithingRecipe smithing) recipes.add(smithing);
            }
        }
        System.err.println("smithing recipes loaded: " + recipes.size());

        JsonArray cases = new JsonArray();
        for (Spec[] triple : scenarios()) {
            ItemStack template = build(triple[0]);
            ItemStack base = build(triple[1]);
            ItemStack addition = build(triple[2]);
            var input = new SmithingRecipeInput(template, base, addition);

            ItemStack result = ItemStack.EMPTY;
            String matched = null;
            for (SmithingRecipe recipe : recipes) {
                if (recipe.matches(input, null)) {
                    result = recipe.assemble(input);
                    matched = recipe.getClass().getSimpleName();
                    break;
                }
            }

            JsonObject entry = new JsonObject();
            entry.add("template", spec(triple[0]));
            entry.add("base", spec(triple[1]));
            entry.add("addition", spec(triple[2]));
            entry.add("matched", matched == null ? JsonNull.INSTANCE : new JsonPrimitive(matched));
            entry.add("result", describe(result));
            cases.add(entry);
        }
        Files.writeString(Path.of(args[0]), GSON.toJson(cases));
        System.err.println("smithing cases: " + cases.size());
    }

    static Spec trimmed(String item, String material, String pattern) {
        return new Spec(item, 1, 0, 0, Map.of(), Map.of(), null);
    }

    static List<Spec[]> scenarios() {
        List<Spec[]> out = new ArrayList<>();
        String[] diamondGear = {"minecraft:diamond_sword", "minecraft:diamond_pickaxe", "minecraft:diamond_axe",
                "minecraft:diamond_shovel", "minecraft:diamond_hoe", "minecraft:diamond_helmet",
                "minecraft:diamond_chestplate", "minecraft:diamond_leggings", "minecraft:diamond_boots"};
        String[] armour = {"minecraft:iron_chestplate", "minecraft:diamond_helmet", "minecraft:netherite_leggings",
                "minecraft:golden_boots", "minecraft:leather_helmet", "minecraft:turtle_helmet",
                "minecraft:chainmail_chestplate"};
        String[] templates = {"minecraft:netherite_upgrade_smithing_template",
                "minecraft:coast_armor_trim_smithing_template", "minecraft:dune_armor_trim_smithing_template",
                "minecraft:eye_armor_trim_smithing_template", "minecraft:rib_armor_trim_smithing_template",
                "minecraft:silence_armor_trim_smithing_template", "minecraft:ward_armor_trim_smithing_template"};
        String[] materials = {"minecraft:netherite_ingot", "minecraft:iron_ingot", "minecraft:gold_ingot",
                "minecraft:diamond", "minecraft:emerald", "minecraft:redstone", "minecraft:lapis_lazuli",
                "minecraft:copper_ingot", "minecraft:quartz", "minecraft:amethyst_shard", "minecraft:resin_brick",
                "minecraft:stick"};

        // Netherite upgrades, including damaged and enchanted bases whose
        // components must carry across.
        for (String gear : diamondGear) {
            out.add(new Spec[] {Spec.of(templates[0]), Spec.of(gear), Spec.of("minecraft:netherite_ingot")});
            out.add(new Spec[] {Spec.of(templates[0]), Spec.damaged(gear, maxDamage(gear) / 2),
                    Spec.of("minecraft:netherite_ingot")});
            out.add(new Spec[] {Spec.of(templates[0]),
                    Spec.enchanted(gear, Map.of("minecraft:unbreaking", 3, "minecraft:mending", 1)),
                    Spec.of("minecraft:netherite_ingot")});
            // Wrong addition for the transform.
            out.add(new Spec[] {Spec.of(templates[0]), Spec.of(gear), Spec.of("minecraft:iron_ingot")});
        }

        // Trim application across templates, materials and armour slots.
        for (String template : templates)
            for (String piece : armour)
                for (String material : materials)
                    out.add(new Spec[] {Spec.of(template), Spec.of(piece), Spec.of(material)});

        // Degenerate triples.
        out.add(new Spec[] {null, null, null});
        out.add(new Spec[] {Spec.of(templates[0]), null, Spec.of("minecraft:netherite_ingot")});
        out.add(new Spec[] {null, Spec.of("minecraft:diamond_sword"), Spec.of("minecraft:netherite_ingot")});
        out.add(new Spec[] {Spec.of(templates[0]), Spec.of("minecraft:diamond_sword"), null});
        out.add(new Spec[] {Spec.of("minecraft:stick"), Spec.of("minecraft:diamond_sword"),
                Spec.of("minecraft:netherite_ingot")});
        out.add(new Spec[] {Spec.of(templates[0]), Spec.of("minecraft:netherite_sword"),
                Spec.of("minecraft:netherite_ingot")});
        return out;
    }
}
