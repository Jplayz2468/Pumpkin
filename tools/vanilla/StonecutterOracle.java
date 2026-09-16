import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.crafting.*;

/**
 * Records, for every item that any stonecutting recipe accepts, the ordered
 * list of offered results.
 *
 * Order is load order: the recipe map is built from the datapack sorted by
 * resource key, and the menu offers that order to the client, which selects by
 * index. A different order therefore hands the player a different item for the
 * same button, so the list is compared in order, not as a set.
 */
public class StonecutterOracle extends WorkstationSupport {
    public static void main(String[] args) throws Exception {
        bootstrap();

        // Sorted by resource key, matching how the recipe map is populated.
        List<StonecutterRecipe> recipes = new ArrayList<>();
        Path root = DATA.resolve("recipe");
        try (var files = Files.walk(root)) {
            for (var path : files.filter(p -> p.toString().endsWith(".json")).sorted().toList()) {
                var json = JsonParser.parseString(Files.readString(path));
                if (!json.isJsonObject()) continue;
                var type = json.getAsJsonObject().get("type");
                if (type == null || !type.getAsString().equals("minecraft:stonecutting")) continue;
                if (Recipe.CODEC.parse(ops, json).getOrThrow() instanceof StonecutterRecipe cutting)
                    recipes.add(cutting);
            }
        }
        System.err.println("stonecutting recipes loaded: " + recipes.size());

        JsonArray cases = new JsonArray();
        for (Item item : BuiltInRegistries.ITEM) {
            ItemStack input = new ItemStack(item);
            var single = new SingleRecipeInput(input);
            JsonArray offered = new JsonArray();
            for (StonecutterRecipe recipe : recipes) {
                if (!recipe.matches(single, null)) continue;
                JsonObject result = new JsonObject();
                ItemStack assembled = recipe.assemble(single);
                result.addProperty("item", BuiltInRegistries.ITEM.getKey(assembled.getItem()).toString());
                result.addProperty("count", assembled.getCount());
                offered.add(result);
            }
            if (offered.isEmpty()) continue;
            JsonObject entry = new JsonObject();
            entry.addProperty("input", BuiltInRegistries.ITEM.getKey(item).toString());
            entry.add("offered", offered);
            cases.add(entry);
        }
        Files.writeString(Path.of(args[0]), GSON.toJson(cases));
        System.err.println("stonecutter inputs: " + cases.size());
    }
}
