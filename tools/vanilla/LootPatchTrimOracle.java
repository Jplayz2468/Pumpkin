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
import net.minecraft.world.item.*;
import net.minecraft.world.item.equipment.trim.*;
import net.minecraft.util.RandomSource;
import net.minecraft.util.context.*;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.storage.loot.*;

/** Canonical component patch export plus actual Java application, wire and hash fixtures. */
public class LootPatchTrimOracle {
 static final Gson G=new Gson(); static final Path DATA=Path.of("assets/datapacks/26_2/data/minecraft");
 static JsonObject obj(Object... p){JsonObject o=new JsonObject();for(int i=0;i<p.length;i+=2)o.add((String)p[i],G.toJsonTree(p[i+1]));return o;}
 static JsonArray arr(Object... v){JsonArray a=new JsonArray();for(Object x:v)a.add(G.toJsonTree(x));return a;}
 static JsonObject set(JsonObject p){return obj("function","minecraft:set_components","components",p);}
 static JsonObject count(int n){return obj("function","minecraft:set_count","count",n);}
 static JsonObject table(Object... f){return obj("pools",arr(obj("rolls",1,"entries",arr(obj("type","minecraft:dynamic","name","minecraft:input","functions",arr(f))))));}
 static JsonArray bytes(Tag tag)throws Exception{var b=new ByteArrayOutputStream();NbtIo.writeAnyTag(tag,new DataOutputStream(b));JsonArray a=new JsonArray();for(byte v:b.toByteArray())a.add(v&255);return a;}
 static <T> MappedRegistry<T> registry(ResourceKey<? extends Registry<T>> key,String dir,Codec<T> codec,RegistryOps<JsonElement> ops)throws Exception{var r=new MappedRegistry<T>(key,Lifecycle.stable());try(var files=Files.list(DATA.resolve(dir))){for(var p:files.sorted().toList())r.register(ResourceKey.create(key,Identifier.withDefaultNamespace(p.getFileName().toString().replace(".json",""))),codec.parse(ops,JsonParser.parseString(Files.readString(p))).getOrThrow(),RegistrationInfo.BUILT_IN);}r.freeze();return r;}
 static void collect(JsonElement e,List<JsonObject> patches){if(e.isJsonObject()){var o=e.getAsJsonObject();if(o.has("function")&&o.get("function").getAsString().equals("minecraft:set_components")){var p=o.getAsJsonObject("components");if(!patches.contains(p))patches.add(p);}for(var v:o.entrySet())collect(v.getValue(),patches);}else if(e.isJsonArray())for(var v:e.getAsJsonArray())collect(v,patches);}
 public static void main(String[] args)throws Exception{
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var builtins=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);var baseOps=builtins.createSerializationContext(JsonOps.INSTANCE);
  var materials=registry(Registries.TRIM_MATERIAL,"trim_material",TrimMaterial.DIRECT_CODEC,baseOps);var patterns=registry(Registries.TRIM_PATTERN,"trim_pattern",TrimPattern.DIRECT_CODEC,baseOps);List<Registry<?>> regs=new ArrayList<>();BuiltInRegistries.REGISTRY.forEach(regs::add);regs.add(materials);regs.add(patterns);var access=new RegistryAccess.ImmutableRegistryAccess(regs).freeze();var ops=access.createSerializationContext(JsonOps.INSTANCE);var nbt=access.createSerializationContext(NbtOps.INSTANCE);
  var defaults=JsonParser.parseString(Files.readString(Path.of("assets/items.json"))).getAsJsonObject();
  List<Item> inputs=List.of(Items.STONE,Items.DIAMOND_SWORD,Items.BUNDLE,Items.CROSSBOW,Items.CHEST,Items.ARROW);
  for(Item item:inputs){var v=defaults.getAsJsonObject(BuiltInRegistries.ITEM.getKey(item).getPath()).getAsJsonObject("components");var b=DataComponentMap.builder();for(String key:List.of("max_stack_size","max_damage","damage","container","bundle_contents","charged_projectiles"))if(v.has("minecraft:"+key)){var type=BuiltInRegistries.DATA_COMPONENT_TYPE.getValue(Identifier.withDefaultNamespace(key));bind(b,type,v.get("minecraft:"+key),ops);}item.builtInRegistryHolder().bindComponents(b.build());}
  List<JsonObject> patches=new ArrayList<>();try(var files=Files.walk(DATA.resolve("loot_table"))){for(var p:files.filter(p->p.toString().endsWith(".json")).sorted().toList())collect(JsonParser.parseString(Files.readString(p)),patches);}
  patches.addAll(List.of(obj(),obj("minecraft:damage",3,"minecraft:custom_name",obj("text","patched","bold",true)),obj("minecraft:max_stack_size",1),obj("minecraft:max_stack_size",64,"minecraft:max_damage",10),obj("minecraft:max_stack_size",1,"minecraft:max_damage",100),obj("!minecraft:max_damage",obj(),"minecraft:max_stack_size",64),obj("!minecraft:max_stack_size",obj(),"minecraft:max_damage",10),obj("!minecraft:trim",obj()),obj("minecraft:container",arr(obj("slot",2,"item",obj("id","minecraft:stone","count",65)))),obj("minecraft:container",arr(obj("slot",2,"item",obj("id","minecraft:stone","count",64)))),obj("minecraft:container",arr()),obj("minecraft:bundle_contents",arr()),obj("minecraft:bundle_contents",arr(obj("id","minecraft:diamond_sword"),obj("id","minecraft:diamond_sword"))),obj("minecraft:bundle_contents",arr(obj("id","minecraft:stone","count",65))),obj("minecraft:charged_projectiles",arr(obj("id","minecraft:arrow","count",65))),obj("minecraft:charged_projectiles",arr(obj("id","minecraft:arrow","count",64)))));
  JsonArray fractional=new JsonArray();for(int p:new int[]{3,5,7,11,13,17,19,23,29})fractional.add(obj("id","minecraft:stone","components",obj("minecraft:max_stack_size",p)));patches.add(obj("minecraft:bundle_contents",fractional));
  JsonArray tables=new JsonArray();for(var p:patches)tables.add(table(set(p)));tables.add(table(count(0),set(patches.get(5)),count(1)));tables.add(table(count(130),set(patches.get(0))));tables.add(table(set(patches.get(0)),count(2)));tables.add(table(set(patches.get(0)),set(obj("!minecraft:trim",obj()))));
  JsonArray cache=new JsonArray();for(var p:patches){var parsed=DataComponentPatch.CODEC.parse(ops,p).getOrThrow();cache.add(obj("components",p,"nbt",bytes(DataComponentPatch.CODEC.encodeStart(nbt,parsed).getOrThrow())));}
  Files.writeString(Path.of("assets/loot_component_patches.json"),G.toJson(cache));Files.writeString(Path.of("crates/pumpkin/src/world/loot_patch_tables.json"),G.toJson(tables));
  List<LootTable> compiled=new ArrayList<>();for(var t:tables)compiled.add(LootTable.DIRECT_CODEC.parse(ops,t).getOrThrow());var ctor=LootContext.class.getDeclaredConstructor(LootParams.class,RandomSource.class,HolderGetter.Provider.class);ctor.setAccessible(true);JsonArray cases=new JsonArray();
  for(int t=0;t<tables.size();t++)for(int kind=0;kind<2;kind++)for(Item item:inputs)for(int count:new int[]{1,2,65}){long seed=t*262L+count;RandomSource rng=kind==0?RandomSource.create(seed):new XoroshiroRandomSource(seed);ItemStack input=new ItemStack(item,count);input.set(DataComponents.CUSTOM_NAME,net.minecraft.network.chat.Component.literal("old"));LootParams params=new LootParams(null,new ContextMap.Builder().create(new ContextKeySet.Builder().build()),Map.of(Identifier.parse("minecraft:input"),out->out.accept(input.copy())),0);LootContext ctx=ctor.newInstance(params,rng,access);JsonArray output=new JsonArray();compiled.get(t).getRandomItemsRaw(ctx,s->{try{output.add(obj("count",s.getCount(),"patch",bytes(DataComponentPatch.CODEC.encodeStart(nbt,s.getComponentsPatch()).getOrThrow())));}catch(Exception e){throw new RuntimeException(e);}});cases.add(obj("table",t,"kind",kind,"seed",seed,"item",BuiltInRegistries.ITEM.getKey(item).toString(),"count",count,"output",output,"next",rng.nextLong()));}
  Files.writeString(Path.of("crates/pumpkin/src/world/loot_patch_cases.json"),G.toJson(cases));
  JsonArray wire=new JsonArray();for(var m:materials.listElements().toList())for(var p:patterns.listElements().toList())wire(wire,obj("material",m.unwrapKey().orElseThrow().identifier().toString(),"pattern",p.unwrapKey().orElseThrow().identifier().toString()),access,ops,nbt);
  Object inlineMaterial=obj("asset_name","custom","override_armor_assets",obj("minecraft:iron","custom_iron"),"description",obj("text","Custom material","color","gold"));Object inlinePattern=obj("asset_id","example:pattern","description",obj("text","Custom pattern","bold",true),"decal",true);
  wire(wire,obj("material",inlineMaterial,"pattern",inlinePattern),access,ops,nbt);wire(wire,obj("material",inlineMaterial,"pattern","minecraft:bolt"),access,ops,nbt);wire(wire,obj("material","minecraft:copper","pattern",inlinePattern),access,ops,nbt);
  Files.writeString(Path.of("crates/pumpkin/src/world/trim_wire_cases.json"),G.toJson(wire));System.out.println("tables="+tables.size()+" cases="+cases.size()+" trim wire="+wire.size());
 }
 @SuppressWarnings({"unchecked","rawtypes"})static void bind(DataComponentMap.Builder b,DataComponentType type,JsonElement v,RegistryOps<JsonElement> ops){b.set(type,type.codec().parse(ops,v).getOrThrow());}
 static void wire(JsonArray out,JsonObject json,RegistryAccess access,RegistryOps<JsonElement> ops,RegistryOps<Tag> nbt)throws Exception{var type=DataComponents.TRIM;var value=type.codec().parse(ops,json).getOrThrow();var buf=new net.minecraft.network.RegistryFriendlyByteBuf(Unpooled.buffer(),access);type.streamCodec().encode(buf,value);JsonArray bytes=new JsonArray();while(buf.isReadable())bytes.add(buf.readUnsignedByte());buf.release();out.add(obj("nbt",bytes(type.codec().encodeStart(nbt,value).getOrThrow()),"bytes",bytes,"hash",type.codec().encodeStart(access.createSerializationContext(net.minecraft.util.HashOps.CRC32C_INSTANCE),value).getOrThrow().asInt(),"reference",json.get("material").isJsonPrimitive()&&json.get("pattern").isJsonPrimitive()));}
}
