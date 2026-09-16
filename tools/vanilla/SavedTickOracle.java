import com.google.gson.*;
import java.util.*;
import net.minecraft.core.BlockPos;
import net.minecraft.world.ticks.*;

/** Unmodified Java 26.2 saved/unpacked queues, including overdue work and restarts. */
public class SavedTickOracle {
 public static void main(String[] args) {
  Random random = new Random(26209); JsonArray cases = new JsonArray();
  for (int c = 0; c < 64; c++) {
   List<SavedTick<String>> saved = new ArrayList<>();
   JsonObject test = new JsonObject(); JsonArray input = new JsonArray();
   for (int i = 0; i < 24; i++) {
    int delay = random.nextInt(40) - 20, priority = random.nextInt(7) - 3;
    saved.add(new SavedTick<>("tick", new BlockPos(i % 16, 64 + i, 0), delay, TickPriority.byValue(priority)));
    JsonArray row = new JsonArray(); row.add(i); row.add(delay); row.add(priority); input.add(row);
   }
   long initial = 1_000_000L + random.nextInt(10000);
   LevelChunkTicks<String> queue = new LevelChunkTicks<>(saved); queue.unpack(initial);
   test.addProperty("initial", initial); test.add("ticks", input);
   JsonArray steps = new JsonArray();
   for (int step = 0; step < 6; step++) {
    boolean reload = step == 3;
    long now = initial + new int[]{0, 1, 10, 100000, 100010, 100100}[step];
    if (reload) { saved = queue.pack(initial + 10); queue = new LevelChunkTicks<>(saved); queue.unpack(now); }
    JsonObject trace = new JsonObject(); trace.addProperty("now", now); trace.addProperty("reload", reload);
    JsonArray packed = new JsonArray();
    for (SavedTick<String> tick : queue.pack(now)) { JsonArray row = new JsonArray(); row.add(tick.pos().getY() - 64); row.add(tick.delay()); row.add(tick.priority().getValue()); packed.add(row); }
    trace.add("saved", packed);
    int budget = step == 5 ? 100 : random.nextInt(8); trace.addProperty("budget", budget);
    JsonArray due = new JsonArray();
    while (budget-- > 0 && queue.peek() != null && queue.peek().triggerTick() <= now) due.add(queue.poll().pos().getY() - 64);
    trace.add("due", due); steps.add(trace);
   }
   test.add("steps", steps); cases.add(test);
  }
  System.out.println(cases);
 }
}
