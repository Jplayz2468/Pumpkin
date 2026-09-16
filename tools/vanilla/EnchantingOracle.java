import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.tags.EnchantmentTags;
import net.minecraft.util.RandomSource;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.enchantment.*;

/**
 * Reproduces the enchanting table's offer for a fixed seed, bookshelf count and
 * item, recording the three level costs and the three clues.
 *
 * {@code EnchantmentMenu.slotsChanged} needs a live level to count bookshelves,
 * so its RNG sequence is transcribed here: one shared random seeded with the
 * enchantment seed for the cost pass, reseeded to {@code seed + slot} for each
 * clue. Every cost, selection and draw is the genuine vanilla call, and the
 * transcribed part is the ten lines below.
 *
 * Clues record both the numeric registry id and the enchantment name, so a
 * mismatch distinguishes a wrong enchantment from a wrong id numbering.
 */
public class EnchantingOracle extends WorkstationSupport {
    public static void main(String[] args) throws Exception {
        bootstrap();

        var holders = registries.lookupOrThrow(Registries.ENCHANTMENT).asHolderIdMap();
        var table = enchantments.get(EnchantmentTags.IN_ENCHANTING_TABLE).orElseThrow();
        RandomSource random = RandomSource.create();

        String[] items = {"minecraft:book", "minecraft:diamond_sword", "minecraft:iron_pickaxe",
                "minecraft:diamond_chestplate", "minecraft:bow", "minecraft:fishing_rod", "minecraft:trident",
                "minecraft:golden_helmet", "minecraft:leather_boots", "minecraft:netherite_axe",
                "minecraft:shears", "minecraft:stone"};
        int[] shelves = {0, 1, 2, 5, 8, 15};
        int[] seeds = {0, 1, 7, 262, 1337, -1, 99991, Integer.MAX_VALUE, Integer.MIN_VALUE};

        JsonArray cases = new JsonArray();
        for (String id : items) {
            Item item = BuiltInRegistries.ITEM.get(Identifier.parse(id)).orElseThrow().value();
            ItemStack stack = new ItemStack(item);
            for (int bookcases : shelves)
                for (int seed : seeds) {
                    int[] costs = new int[3];
                    int[] clueId = {-1, -1, -1};
                    int[] clueLevel = {-1, -1, -1};
                    String[] clueName = {null, null, null};

                    if (stack.isEnchantable()) {
                        random.setSeed(seed);
                        for (int i = 0; i < 3; i++) {
                            costs[i] = EnchantmentHelper.getEnchantmentCost(random, i, bookcases, stack);
                            if (costs[i] < i + 1) costs[i] = 0;
                        }
                        for (int i = 0; i < 3; i++) {
                            if (costs[i] <= 0) continue;
                            random.setSeed(seed + i);
                            List<EnchantmentInstance> list =
                                    EnchantmentHelper.selectEnchantment(random, stack, costs[i], table.stream());
                            if (stack.is(Items.BOOK) && list.size() > 1)
                                list.remove(random.nextInt(list.size()));
                            if (list.isEmpty()) continue;
                            EnchantmentInstance chosen = list.get(random.nextInt(list.size()));
                            clueId[i] = holders.getId(chosen.enchantment());
                            clueLevel[i] = chosen.level();
                            clueName[i] = chosen.enchantment().unwrapKey().orElseThrow().identifier().toString();
                        }
                    }

                    JsonObject entry = new JsonObject();
                    entry.addProperty("item", id);
                    entry.addProperty("bookshelves", bookcases);
                    entry.addProperty("seed", seed);
                    entry.add("costs", GSON.toJsonTree(costs));
                    entry.add("clue_id", GSON.toJsonTree(clueId));
                    entry.add("clue_level", GSON.toJsonTree(clueLevel));
                    entry.add("clue_name", GSON.toJsonTree(clueName));
                    cases.add(entry);
                }
        }
        Files.writeString(Path.of(args[0]), GSON.toJson(cases));
        System.err.println("enchanting cases: " + cases.size());
    }
}
