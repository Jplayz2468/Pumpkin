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
import net.minecraft.resources.*;
import net.minecraft.nbt.*;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.sounds.*;
import net.minecraft.world.item.*;
import net.minecraft.advancements.predicates.ItemPredicate;
public class JukeboxComponentOracle {
 static final Gson G=new Gson();
 static JsonObject obj(Object... p){JsonObject o=new JsonObject();for(int i=0;i<p.length;i+=2)o.add((String)p[i],G.toJsonTree(p[i+1]));return o;}
 static JsonArray arr(Object... v){JsonArray a=new JsonArray();for(Object x:v)a.add(G.toJsonTree(x));return a;}
 static JsonArray bytes(Tag tag)throws Exception{var b=new ByteArrayOutputStream();NbtIo.writeAnyTag(tag,new DataOutputStream(b));JsonArray a=new JsonArray();for(byte v:b.toByteArray())a.add(v&255);return a;}
 static void add(JsonArray out,Holder<JukeboxSong> holder,RegistryAccess access,List<ItemPredicate> predicates)throws Exception {
  var component=new JukeboxPlayable(holder);var value=holder.value();var type=DataComponents.JUKEBOX_PLAYABLE;
  var encoded=type.codec().encodeStart(access.createSerializationContext(NbtOps.INSTANCE),component);
  var buf=new RegistryFriendlyByteBuf(Unpooled.buffer(),access);type.streamCodec().encode(buf,component);JsonArray wire=new JsonArray();while(buf.isReadable())wire.add(buf.readUnsignedByte());buf.release();
  var result=obj("wire",wire,"persistent",encoded.isSuccess(),"registry_id",access.lookupOrThrow(Registries.JUKEBOX_SONG).getId(value),"length_ticks",value.lengthInTicks(),"comparator",value.comparatorOutput(),"seconds_bits",Float.floatToRawIntBits(value.lengthInSeconds()));
  if(encoded.isSuccess()){result.add("nbt",bytes(encoded.getOrThrow()));result.addProperty("hash",type.codec().encodeStart(access.createSerializationContext(net.minecraft.util.HashOps.CRC32C_INSTANCE),component).getOrThrow().asInt());}
  JsonArray resumed=new JsonArray();for(long tick:new long[]{-1,0,(long)(value.lengthInTicks()+20)-1,(long)(value.lengthInTicks()+20),(long)(value.lengthInTicks()+20)+1,Long.MAX_VALUE}){var player=new JukeboxSongPlayer(()->{},BlockPos.ZERO);player.setSongWithoutPlaying(holder,tick);resumed.add(obj("tick",tick,"playing",player.isPlaying()));}result.add("resume",resumed);
  ItemStack stack=new ItemStack(Items.STICK);stack.set(type,component);
  result.addProperty("item_persistent",ItemStack.CODEC.encodeStart(access.createSerializationContext(NbtOps.INSTANCE),stack).isSuccess());
  var saved=net.minecraft.world.level.storage.TagValueOutput.createWithContext(net.minecraft.util.ProblemReporter.DISCARDING,access);saved.store("RecordItem",ItemStack.CODEC,stack);result.add("saved_record",bytes(saved.buildResult()));

  var outer=new ItemStack(Items.STICK);outer.set(DataComponents.BUNDLE_CONTENTS,new net.minecraft.world.item.component.BundleContents(List.of(ItemStackTemplate.fromNonEmptyStack(stack))));
  result.addProperty("bundle_persistent",ItemStack.CODEC.encodeStart(access.createSerializationContext(NbtOps.INSTANCE),outer).isSuccess());
  outer=new ItemStack(Items.STICK);outer.set(DataComponents.CONTAINER,net.minecraft.world.item.component.ItemContainerContents.fromItems(List.of(stack)));
  result.addProperty("container_persistent",ItemStack.CODEC.encodeStart(access.createSerializationContext(NbtOps.INSTANCE),outer).isSuccess());
  JsonArray matches=new JsonArray();for(var p:predicates)matches.add(p.test(stack));result.add("matches",matches);out.add(result);
 }
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var base=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
  var songs=LootPatchTrimOracle.registry(Registries.JUKEBOX_SONG,"jukebox_song",JukeboxSong.DIRECT_CODEC,base.createSerializationContext(JsonOps.INSTANCE));
  List<Registry<?>> all=new ArrayList<>();BuiltInRegistries.REGISTRY.forEach(all::add);all.add(songs);var access=new RegistryAccess.ImmutableRegistryAccess(all).freeze();var json=access.createSerializationContext(JsonOps.INSTANCE);var nbt=access.createSerializationContext(NbtOps.INSTANCE);
  JsonArray predicateNbt=new JsonArray();List<ItemPredicate> predicates=new ArrayList<>();
  for(var input:List.of(obj("predicates",obj("minecraft:jukebox_playable",obj())),obj("predicates",obj("minecraft:jukebox_playable",obj("song","minecraft:cat"))),obj("predicates",obj("minecraft:jukebox_playable",obj("song",arr("minecraft:13","minecraft:cat")))),obj("predicates",obj("minecraft:jukebox_playable",obj("song",arr()))),obj("components",obj("minecraft:jukebox_playable","minecraft:cat")))) {var p=ItemPredicate.CODEC.parse(json,input).getOrThrow();predicates.add(p);predicateNbt.add(bytes(ItemPredicate.CODEC.encodeStart(nbt,p).getOrThrow()));}
  Items.STICK.builtInRegistryHolder().bindComponents(DataComponentMap.EMPTY);
  JsonArray cases=new JsonArray();for(var song:songs.listElements().toList())add(cases,song,access,predicates);
  for(float length:new float[]{0.001f,1.001f,12.25f,0,-1,-2,Float.MAX_VALUE,Float.POSITIVE_INFINITY,Float.NEGATIVE_INFINITY,Float.NaN})
   add(cases,Holder.direct(new JukeboxSong(SoundEvents.MUSIC_DISC_CAT,Component.literal("Inline"),length,7)),access,predicates);
  for(Float range:new Float[]{null,0.0f,12.5f}) add(cases,Holder.direct(new JukeboxSong(Holder.direct(range==null?SoundEvent.createVariableRangeEvent(Identifier.parse("example:song")):SoundEvent.createFixedRangeEvent(Identifier.parse("example:song"),range)),Component.literal("Custom sound"),2.75f,15)),access,predicates);
  JsonArray codec=new JsonArray();for(var input:List.of(new JsonPrimitive("cat"),new JsonPrimitive(":cat"),new JsonPrimitive("Minecraft:cat"),new JsonPrimitive("minecraft:ca t"),new JsonPrimitive("minecraft:cat"),new JsonPrimitive("example:cat"),new JsonPrimitive("minecraft:missing"),obj("song","minecraft:cat"),obj("sound_event","minecraft:music_disc.cat","description","Inline","length_in_seconds",2.5,"comparator_output",4)))codec.add(obj("nbt",bytes(JsonOps.INSTANCE.convertTo(NbtOps.INSTANCE,input)),"valid",DataComponents.JUKEBOX_PLAYABLE.codec().parse(json,input).isSuccess()));
  Files.writeString(Path.of("crates/pumpkin/src/item/jukebox_component_cases.json"),G.toJson(obj("cases",cases,"predicates",predicateNbt,"codec",codec)));System.out.println("jukebox cases="+cases.size()+" predicates="+predicates.size());
 }
}
