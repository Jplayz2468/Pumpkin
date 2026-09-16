import com.google.gson.*;
import com.mojang.serialization.*;
import java.nio.file.*;
import java.io.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.network.chat.*;
import net.minecraft.nbt.*;
import net.minecraft.util.HashOps;
public class TextArgumentOracle {
 static final Gson G=new Gson();
 static JsonArray bytes(Tag tag)throws Exception{var b=new ByteArrayOutputStream();NbtIo.writeAnyTag(tag,new DataOutputStream(b));JsonArray a=new JsonArray();for(byte v:b.toByteArray())a.add(v&255);return a;}
 static JsonObject result(Component c)throws Exception {var o=new JsonObject();o.add("json",ComponentSerialization.CODEC.encodeStart(JsonOps.INSTANCE,c).getOrThrow());o.add("nbt",bytes(ComponentSerialization.CODEC.encodeStart(NbtOps.INSTANCE,c).getOrThrow()));o.addProperty("hash",ComponentSerialization.CODEC.encodeStart(HashOps.CRC32C_INSTANCE,c).getOrThrow().asInt());o.addProperty("text",c.getString());return o;}
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();JsonArray cases=new JsonArray();
  for(String text:new String[]{"%s", "%2$s/%1$s/%s/%%", "plain", "bad %d", "%0$s", "%999999999999999$s", "trail %", "%s %s %s", "café %1$s 🐝", "\\%s"})
  for(String values:new String[]{"[]","[true,false]","[1,128,32768,2147483648,0.5,0.1]","[\"one\",{\"text\":\"two\",\"bold\":true}]"}){
   JsonObject in=new JsonObject();in.addProperty("translate","example.missing");in.addProperty("fallback",text);in.add("with",JsonParser.parseString(values));var c=ComponentSerialization.CODEC.parse(JsonOps.INSTANCE,in).getOrThrow();var o=new JsonObject();o.add("input",in);o.add("direct",result(c));var tag=ComponentSerialization.CODEC.encodeStart(NbtOps.INSTANCE,c).getOrThrow();o.add("restored",result(ComponentSerialization.CODEC.parse(NbtOps.INSTANCE,tag).getOrThrow()));cases.add(o);
  }
  for(String input:new String[]{"{\"translate\":\"item.Minecraft.stone\"}","{\"translate\":\"container.isLocked\",\"with\":[\"Chest\"]}","{\"translate\":\"container.islocked\"}","{\"translate\":\"example.MissingCase\"}","{\"translate\":\"example.missing\",\"fallback\":7}","{\"translate\":\"item.minecraft.stone\",\"fallback\":\"wrong\"}","{\"translate\":\"example.missing\",\"fallback\":\"%s\",\"with\":[{\"translate\":\"example.inner\",\"fallback\":\"%s\",\"with\":[0.25]}],\"extra\":[{\"translate\":\"example.suffix\",\"fallback\":\"/%s\",\"with\":[false]}]}"}) {
   var in=JsonParser.parseString(input);var c=ComponentSerialization.CODEC.parse(JsonOps.INSTANCE,in).getOrThrow();var o=new JsonObject();o.add("input",in);o.add("direct",result(c));var tag=ComponentSerialization.CODEC.encodeStart(NbtOps.INSTANCE,c).getOrThrow();o.add("restored",result(ComponentSerialization.CODEC.parse(NbtOps.INSTANCE,tag).getOrThrow()));cases.add(o);
  }
  for(Tag value:typedArguments()) {var tag=new CompoundTag();tag.putString("translate","example.missing");tag.putString("fallback","%s");var list=new ListTag();list.add(value);tag.put("with",list);var c=ComponentSerialization.CODEC.parse(NbtOps.INSTANCE,tag).getOrThrow();var o=new JsonObject();o.add("input_nbt",bytes(tag));o.add("restored",result(c));cases.add(o);}
  Files.writeString(Path.of("crates/pumpkin/src/world/text_argument_cases.json"),G.toJson(cases));System.out.println("text cases="+cases.size());
 }
 static java.util.List<Tag> typedArguments(){
  var out=new java.util.ArrayList<Tag>();out.addAll(java.util.List.of(ByteTag.valueOf((byte)7),ShortTag.valueOf((short)7),IntTag.valueOf(7),LongTag.valueOf(7),FloatTag.valueOf(7),DoubleTag.valueOf(7)));
  for(float v:new float[]{Float.MIN_VALUE,Float.MIN_NORMAL,Float.MAX_VALUE,-0.0f,0.001f,0.0001f,1.0e7f,9999999.0f})out.add(FloatTag.valueOf(v));
  for(double v:new double[]{Double.MIN_VALUE,Double.MIN_NORMAL,Double.MAX_VALUE,-0.0,0.001,0.0001,1.0e7,9999999.0})out.add(DoubleTag.valueOf(v));
  return out;
 }
}
