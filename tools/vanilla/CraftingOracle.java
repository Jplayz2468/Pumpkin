import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.crafting.*;

/**
 * Builds a canonical input grid for every built-in crafting recipe, resolves it
 * against the real Java 26.2 recipe list and records the first match, its
 * assembled output and its remaining items.
 *
 * Each ingredient contributes its first accepted item, chosen by registry order,
 * so the grid is deterministic. The recorded match is the first recipe that
 * accepts the grid in load order, which is what a player gets when two recipes
 * overlap, so the Rust side must agree on the winner as well as the output.
 */
public class CraftingOracle extends WorkstationSupport {
    record Loaded(String id, Recipe<?> recipe) {}

    public static void main(String[] args) throws Exception {
        bootstrap();

        List<Loaded> recipes = new ArrayList<>();
        Path root = DATA.resolve("recipe");
        try (var files = Files.walk(root)) {
            for (var path : files.filter(p -> p.toString().endsWith(".json")).sorted().toList()) {
                var json = JsonParser.parseString(Files.readString(path));
                if (!json.isJsonObject()) continue;
                var type = json.getAsJsonObject().get("type");
                if (type == null) continue;
                String kind = type.getAsString();
                if (!kind.equals("minecraft:crafting_shaped") && !kind.equals("minecraft:crafting_shapeless")) continue;
                Recipe<?> recipe = Recipe.CODEC.parse(ops, json).getOrThrow();
                recipes.add(new Loaded(root.relativize(path).toString().replace(".json", ""), recipe));
            }
        }
        System.err.println("crafting recipes loaded: " + recipes.size());

        JsonArray cases = new JsonArray();
        int unbuildable = 0;
        for (Loaded loaded : recipes) {
            List<ItemStack> grid;
            int width;
            int height;
            if (loaded.recipe() instanceof ShapedRecipe shaped) {
                width = shaped.getWidth();
                height = shaped.getHeight();
                grid = new ArrayList<>();
                for (Optional<Ingredient> ingredient : shaped.getIngredients()) grid.add(first(ingredient));
            } else if (loaded.recipe() instanceof ShapelessRecipe shapeless) {
                @SuppressWarnings("unchecked")
                List<Ingredient> ingredients =
                        (List<Ingredient>) get(ShapelessRecipe.class, shapeless, "ingredients");
                if (ingredients.size() > 9) { unbuildable++; continue; }
                width = Math.min(3, ingredients.size());
                height = (ingredients.size() + 2) / 3;
                grid = new ArrayList<>();
                for (Ingredient ingredient : ingredients) grid.add(first(Optional.of(ingredient)));
                while (grid.size() < width * height) grid.add(ItemStack.EMPTY);
            } else {
                continue;
            }
            if (grid.stream().anyMatch(stack -> stack == null)) { unbuildable++; continue; }

            var input = CraftingInput.of(width, height, grid);

            String matched = null;
            ItemStack result = ItemStack.EMPTY;
            JsonArray remaining = new JsonArray();
            for (Loaded candidate : recipes) {
                if (!(candidate.recipe() instanceof CraftingRecipe crafting)) continue;
                if (!crafting.matches(input, null)) continue;
                matched = candidate.id();
                result = crafting.assemble(input);
                for (ItemStack stack : crafting.getRemainingItems(input))
                    remaining.add(stack.isEmpty() ? JsonNull.INSTANCE
                            : new JsonPrimitive(BuiltInRegistries.ITEM.getKey(stack.getItem()).toString()));
                break;
            }

            JsonArray encodedGrid = new JsonArray();
            for (ItemStack stack : grid)
                encodedGrid.add(stack.isEmpty() ? JsonNull.INSTANCE
                        : new JsonPrimitive(BuiltInRegistries.ITEM.getKey(stack.getItem()).toString()));

            JsonObject entry = new JsonObject();
            entry.addProperty("recipe", loaded.id());
            entry.addProperty("width", width);
            entry.addProperty("height", height);
            entry.add("grid", encodedGrid);
            entry.add("matched", matched == null ? JsonNull.INSTANCE : new JsonPrimitive(matched));
            entry.add("result", describe(result));
            entry.add("remaining", remaining);
            cases.add(entry);
        }
        Files.writeString(Path.of(args[0]), GSON.toJson(cases));
        System.err.println("crafting cases: " + cases.size() + " (skipped " + unbuildable + ")");
    }

    /** The first accepted item of an ingredient, or empty for an absent one. */
    static ItemStack first(Optional<Ingredient> ingredient) {
        if (ingredient.isEmpty()) return ItemStack.EMPTY;
        Optional<Holder<Item>> item = ingredient.get().items().findFirst();
        return item.map(holder -> new ItemStack(holder.value())).orElse(null);
    }
}
