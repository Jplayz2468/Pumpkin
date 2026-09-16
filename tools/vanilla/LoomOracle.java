import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.core.Holder;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.entity.BannerPattern;

/**
 * Records the loom's ordered selectable-pattern list for an empty pattern slot
 * and for every item that provides banner patterns.
 *
 * Vanilla reads the {@code provides_banner_patterns} component off the held
 * item; the empty slot falls back to the {@code no_item_required} tag. The
 * client selects a pattern by index, so the list is compared in order.
 */
public class LoomOracle extends WorkstationSupport {
    public static void main(String[] args) throws Exception {
        bootstrap();

        JsonArray cases = new JsonArray();

        // Empty pattern slot: the no_item_required tag, in datapack order.
        JsonArray fallback = new JsonArray();
        for (var entry : JsonParser
                .parseString(Files.readString(DATA.resolve("tags/banner_pattern/no_item_required.json")))
                .getAsJsonObject().getAsJsonArray("values")) {
            String id = entry.isJsonObject() ? entry.getAsJsonObject().get("id").getAsString() : entry.getAsString();
            fallback.add(id.contains(":") ? id : "minecraft:" + id);
        }
        JsonObject empty = new JsonObject();
        empty.add("item", JsonNull.INSTANCE);
        empty.add("patterns", fallback);
        cases.add(empty);

        for (Item item : BuiltInRegistries.ITEM) {
            ItemStack stack = new ItemStack(item);
            var provided = stack.get(DataComponents.PROVIDES_BANNER_PATTERNS);
            String key = BuiltInRegistries.ITEM.getKey(item).toString();
            boolean looksLikePattern = key.endsWith("_banner_pattern");
            if (provided == null && !looksLikePattern) continue;

            JsonArray patterns = new JsonArray();
            if (provided != null)
                for (Holder<BannerPattern> holder : provided)
                    patterns.add(holder.unwrapKey().orElseThrow().identifier().toString());

            JsonObject entry = new JsonObject();
            entry.addProperty("item", key);
            entry.add("patterns", patterns);
            cases.add(entry);
        }
        Files.writeString(Path.of(args[0]), GSON.toJson(cases));
        System.err.println("loom pattern sources: " + cases.size());
    }
}
