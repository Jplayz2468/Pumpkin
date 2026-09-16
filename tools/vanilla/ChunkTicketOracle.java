// Exercise the actual 26.2 TicketStorage codec and duplicate activation path.
import com.google.gson.*;
import net.minecraft.nbt.*;
import net.minecraft.world.level.TicketStorage;
public class ChunkTicketOracle {
 public static void main(String[] args) throws Exception {
  var out=System.out;net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
  JsonArray cases=new JsonArray();
  String[] positions={"[I;-4,7]","[L;-4L,7L]","[B;-4B,7B]","[-4.9d,7.9d]","[4294967292L,7L]"};
  String[] levels={"30","30b","30s","30L","30.9f","30.9d"};
  String[] lifetimes={"200L","200","200s","-1.9d","-1.9f","0b"};
  for(String pos:positions) for(String level:levels) for(String life:lifetimes) {
   String input="{tickets:[{chunk_pos:"+pos+",type:\"minecraft:portal\",level:"+level+",ticks_left:"+life+"}]}";
   var storage=TicketStorage.CODEC.parse(NbtOps.INSTANCE,TagParser.parseCompoundFully(input)).getOrThrow();
   storage.activateAllDeactivatedTickets();
   var encoded=TicketStorage.CODEC.encodeStart(NbtOps.INSTANCE,storage).getOrThrow();
   JsonArray row=new JsonArray();row.add(input);row.add(encoded.toString());cases.add(row);
  }
  for(String kind:new String[]{"portal","forced"}) {
   String ticket="{chunk_pos:[I;2,3],type:\"minecraft:"+kind+"\",level:"+(kind.equals("portal")?30:31)+",ticks_left:12L}";
   String input="{tickets:["+ticket+","+ticket+"]}";
   var storage=TicketStorage.CODEC.parse(NbtOps.INSTANCE,TagParser.parseCompoundFully(input)).getOrThrow();storage.activateAllDeactivatedTickets();
   JsonArray row=new JsonArray();row.add(input);row.add(TicketStorage.CODEC.encodeStart(NbtOps.INSTANCE,storage).getOrThrow().toString());cases.add(row);
  }
  out.println(cases);
 }
}
