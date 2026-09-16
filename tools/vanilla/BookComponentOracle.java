import com.google.gson.*;
import com.mojang.serialization.*;
import java.nio.file.*;
import java.util.*;
import java.io.*;
import io.netty.buffer.Unpooled;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.component.*;
import net.minecraft.core.registries.*;
import net.minecraft.nbt.*;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.util.HashOps;
public class BookComponentOracle {
 static final Gson G=new Gson();
 static JsonObject obj(Object... p){JsonObject o=new JsonObject();for(int i=0;i<p.length;i+=2)o.add((String)p[i],G.toJsonTree(p[i+1]));return o;}
 static JsonArray arr(Object... v){JsonArray a=new JsonArray();for(Object x:v)a.add(G.toJsonTree(x));return a;}
 static JsonArray bytes(Tag tag)throws Exception{var b=new ByteArrayOutputStream();NbtIo.writeAnyTag(tag,new DataOutputStream(b));JsonArray a=new JsonArray();for(byte v:b.toByteArray())a.add(v&255);return a;}
 @SuppressWarnings({"unchecked","rawtypes"}) static void add(JsonArray out,DataComponentType type,JsonElement input,RegistryAccess access,boolean exact)throws Exception {
  var json=access.createSerializationContext(JsonOps.INSTANCE);var nbt=access.createSerializationContext(NbtOps.INSTANCE);
  var result=type.codec().parse(json,input);var o=obj("component",BuiltInRegistries.DATA_COMPONENT_TYPE.getKey(type).toString(),"input_nbt",bytes(JsonOps.INSTANCE.convertTo(NbtOps.INSTANCE,input)),"valid",result.isSuccess());
  if(result.isSuccess()) {
   var value=result.getOrThrow();var tag=(Tag)type.codec().encodeStart(nbt,value).getOrThrow();
   // Expected state is the actual saved-NBT decode, including primitive argument widths.
   value=type.codec().parse(nbt,tag).getOrThrow();o.add("nbt",bytes((Tag)type.codec().encodeStart(nbt,value).getOrThrow()));
   o.addProperty("hash",((com.google.common.hash.HashCode)type.codec().encodeStart(access.createSerializationContext(HashOps.CRC32C_INSTANCE),value).getOrThrow()).asInt());
   var buf=new RegistryFriendlyByteBuf(Unpooled.buffer(),access);type.streamCodec().encode(buf,value);JsonArray wire=new JsonArray();while(buf.isReadable())wire.add(buf.readUnsignedByte());buf.release();o.add("wire",wire);o.addProperty("exact_wire",exact);
  }
  out.add(o);
 }
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var access=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);JsonArray out=new JsonArray();
  for(var input:List.of(obj(),obj("pages",arr("")),obj("pages",arr("raw",obj("raw","unfiltered","filtered","safe"))),obj("pages",arr(obj("raw","only"))),obj("pages",arr("a".repeat(1024))),obj("pages",arr("🐝".repeat(512))),obj("pages",arr("a".repeat(1025))),obj("pages",arr("🐝".repeat(513))),obj("pages",arr(obj("filtered","missing"))),obj("pages",arr(7)),obj("pages",false)))add(out,DataComponents.WRITABLE_BOOK_CONTENT,input,access,true);
  for(int size:new int[]{100,101}){JsonArray pages=new JsonArray();for(int i=0;i<size;i++)pages.add("page "+i);add(out,DataComponents.WRITABLE_BOOK_CONTENT,obj("pages",pages),access,true);}
  for(int generation:new int[]{-1,0,1,2,3,4})for(boolean resolved:new boolean[]{false,true})add(out,DataComponents.WRITTEN_BOOK_CONTENT,obj("title","Title","author","Author","generation",generation,"resolved",resolved,"pages",arr("plain",obj("raw","raw","filtered","safe"))),access,true);
  for(var title:List.of("","t".repeat(32),"t".repeat(33),"🐝".repeat(16),"🐝".repeat(17),obj("raw","Title","filtered","Safe"),obj("filtered","missing")))add(out,DataComponents.WRITTEN_BOOK_CONTENT,obj("title",title,"author","Author"),access,true);
  for(var input:List.of(obj(),obj("title","Title"),obj("title","Title","author","Author","pages",arr(obj("raw",obj("text","styled","bold",true),"filtered",obj("text","safe","italic",true)))),obj("title","Title","author","Author","pages",arr(obj("translate","example.key","fallback","%s","with",arr(3)))) ,obj("title","Title","author","Author","pages",arr(obj("raw",false))),obj("title","Title","author","Author","resolved","wrong")))add(out,DataComponents.WRITTEN_BOOK_CONTENT,input,access,false);
  for(int length:new int[]{32765,32766})add(out,DataComponents.WRITTEN_BOOK_CONTENT,obj("title","Title","author","Author","pages",arr("x".repeat(length))),access,true);
  for(int count:new int[]{5460,5461})add(out,DataComponents.WRITTEN_BOOK_CONTENT,obj("title","Title","author","Author","pages",arr("\u2028".repeat(count))),access,true);
  Files.writeString(Path.of("crates/pumpkin-protocol/src/codec/book_component_cases.json"),G.toJson(out));System.out.println("book cases="+out.size());
 }
}
