import com.google.gson.*;
import com.mojang.serialization.JsonOps;
import java.nio.file.*;
import net.minecraft.world.RandomSequences;
import net.minecraft.resources.Identifier;
import net.minecraft.util.RandomSource;
public class RandomSequenceOracle {
 static JsonElement state(RandomSequences s) { return RandomSequences.CODEC.encodeStart(JsonOps.INSTANCE,s).getOrThrow(); }
 static JsonArray draw(RandomSource r) { JsonArray a=new JsonArray(); for(int b:new int[]{1,2,3,7,17,1073741825,2147483647,1073741825,31,1000})for(int i=0;i<8;i++)a.add(r.nextInt(b)); a.add(r.nextLong());return a; }
 public static void main(String[] args)throws Exception {
  JsonArray cases=new JsonArray();
  for(long seed:new long[]{0,1,-1,262,Long.MIN_VALUE,Long.MAX_VALUE})for(int salt:new int[]{0,17,-17,Integer.MIN_VALUE})for(int flags=0;flags<4;flags++) {
   RandomSequences s=new RandomSequences();s.setSeedDefaults(salt,(flags&1)!=0,(flags&2)!=0);
   Identifier id=Identifier.parse("minecraft:blocks/diamond_ore");
   JsonObject c=new JsonObject();c.addProperty("seed",seed);c.addProperty("salt",salt);c.addProperty("flags",flags);
   c.add("draw",draw(s.get(id,seed)));c.add("saved",state(s));
   if(seed==262 && salt==17 && flags==3) {
    var root=new net.minecraft.nbt.CompoundTag();root.putInt("DataVersion",4903);
    root.put("data",RandomSequences.CODEC.encodeStart(net.minecraft.nbt.NbtOps.INSTANCE,s).getOrThrow());
    net.minecraft.nbt.NbtIo.writeCompressed(root,Path.of(args[0]).resolveSibling("random_sequence_java.dat"));
   }
   RandomSequences loaded=RandomSequences.CODEC.parse(JsonOps.INSTANCE,state(s)).getOrThrow();
   c.add("resumed",draw(loaded.get(id,seed)));
   loaded.reset(id,seed);c.add("reset",draw(loaded.get(id,seed)));
   loaded.reset(id,seed,-99,false,true);c.add("override",draw(loaded.get(id,seed)));
   cases.add(c);
  }
  Files.writeString(Path.of(args[0]),new Gson().toJson(cases));
 }
}
