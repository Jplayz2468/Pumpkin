import com.google.gson.*;
import java.util.*;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.ticks.*;
public class ChunkTickOracle {
 public static void main(String[] args) {
  Random rng=new Random(262); JsonArray cases=new JsonArray();
  for(int c=0;c<128;c++) {
   int[] mask={0}; LevelTicks<String> level=new LevelTicks<>(p -> (mask[0]&(1<<ChunkPos.unpack(p).x()))!=0);
   for(int q=0;q<4;q++) level.addContainer(new ChunkPos(q,0),new LevelChunkTicks<>());
   JsonObject test=new JsonObject(); JsonArray ticks=new JsonArray();
   for(int i=0;i<48;i++) {
    int q=rng.nextInt(4), delay=rng.nextInt(8), priority=rng.nextInt(7)-3;
    int x=q*16+i%16, y=64+i; String type="tick"+i;
    level.schedule(new ScheduledTick<>(type,new BlockPos(x,y,0),delay,TickPriority.byValue(priority),i));
    JsonArray row=new JsonArray(); for(int n:new int[]{q,x,y,delay,priority,i})row.add(n); ticks.add(row);
   }
   JsonArray steps=new JsonArray();
   for(int time=0;time<25;time++) {
    mask[0]=time>=12?15:rng.nextInt(16); int budget=time>=12?48:rng.nextInt(9);
    JsonObject step=new JsonObject();step.addProperty("mask",mask[0]);step.addProperty("budget",budget);
    JsonArray output=new JsonArray();level.tick(time,budget,(p,t)->output.add(Integer.parseInt(t.substring(4))));step.add("output",output);steps.add(step);
   }
   test.add("ticks",ticks);test.add("steps",steps);cases.add(test);
  }
  System.out.println(cases);
 }
}
