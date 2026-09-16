import com.google.gson.*;
import com.mojang.serialization.*;
import java.nio.file.*;
import java.util.*;
import java.io.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.component.*;
import net.minecraft.core.registries.*;
import net.minecraft.resources.*;
import net.minecraft.nbt.*;
import net.minecraft.world.item.*;
import net.minecraft.world.item.enchantment.Enchantment;
import net.minecraft.advancements.predicates.ItemPredicate;
public class ItemPredicateOracle {
 static final Gson G=new Gson();
 static JsonObject obj(Object... p){JsonObject o=new JsonObject();for(int i=0;i<p.length;i+=2)o.add((String)p[i],G.toJsonTree(p[i+1]));return o;}
 static JsonArray arr(Object... v){JsonArray a=new JsonArray();for(Object x:v)a.add(G.toJsonTree(x));return a;}
 static JsonObject part(String name,Object value){return obj("predicates",obj("minecraft:"+name,value));}
 static JsonArray bytes(Tag tag)throws Exception{var b=new ByteArrayOutputStream();NbtIo.writeAnyTag(tag,new DataOutputStream(b));JsonArray a=new JsonArray();for(byte v:b.toByteArray())a.add(v&255);return a;}
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  BuiltInRegistries.ITEM.prepareTagReload(new net.minecraft.tags.TagLoader.LoadResult<>(Registries.ITEM,LootEnchantmentOracle.tags("item",BuiltInRegistries.ITEM))).apply();
  var base=net.minecraft.data.registries.VanillaRegistries.createLookup();
  var enchantments=new MappedRegistry<Enchantment>(Registries.ENCHANTMENT,Lifecycle.stable());
  var pending=enchantments.createRegistrationLookup();
  var ops=RegistryOps.create(JsonOps.INSTANCE,new RegistryOps.RegistryInfoLookup(){
   @SuppressWarnings({"unchecked","rawtypes"}) public <T> Optional<RegistryOps.RegistryInfo<T>> lookup(ResourceKey<? extends Registry<? extends T>> key){
    if(key.equals(Registries.ENCHANTMENT))return (Optional)Optional.of(new RegistryOps.RegistryInfo<>(enchantments,pending,Lifecycle.stable()));
    if(key.equals(Registries.ITEM))return (Optional)Optional.of(RegistryOps.RegistryInfo.fromRegistryLookup(BuiltInRegistries.ITEM));
    return base.lookup(key).map(RegistryOps.RegistryInfo::fromRegistryLookup);
   }
  });
  try(var files=Files.list(LootEnchantmentOracle.DATA.resolve("enchantment"))){for(var p:files.sorted().toList()){var value=Enchantment.DIRECT_CODEC.parse(ops,JsonParser.parseString(Files.readString(p))).getOrThrow();enchantments.register(ResourceKey.create(Registries.ENCHANTMENT,Identifier.withDefaultNamespace(p.getFileName().toString().replace(".json",""))),value,RegistrationInfo.BUILT_IN);}}
  enchantments.bindTags(LootEnchantmentOracle.tags("enchantment",enchantments));enchantments.freeze();
  List<Registry<?>> all=new ArrayList<>();BuiltInRegistries.REGISTRY.forEach(all::add);all.add(enchantments);var baseOps=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY).createSerializationContext(JsonOps.INSTANCE);
  all.add(LootPatchTrimOracle.registry(Registries.TRIM_MATERIAL,"trim_material",net.minecraft.world.item.equipment.trim.TrimMaterial.DIRECT_CODEC,baseOps));
  all.add(LootPatchTrimOracle.registry(Registries.TRIM_PATTERN,"trim_pattern",net.minecraft.world.item.equipment.trim.TrimPattern.DIRECT_CODEC,baseOps));
  var registries=new RegistryAccess.ImmutableRegistryAccess(all).freeze();
  var json=registries.createSerializationContext(JsonOps.INSTANCE);var nbt=registries.createSerializationContext(NbtOps.INSTANCE);
  var defaults=JsonParser.parseString(Files.readString(Path.of("assets/items.json"))).getAsJsonObject();
  for(Item item:BuiltInRegistries.ITEM){var raw=defaults.getAsJsonObject(BuiltInRegistries.ITEM.getKey(item).getPath()).getAsJsonObject("components");var b=DataComponentMap.builder();for(String key:List.of("max_stack_size","max_damage","damage","enchantments","stored_enchantments","container","bundle_contents","charged_projectiles","potion_contents","fireworks","firework_explosion","writable_book_content","written_book_content","custom_name","item_name"))if(raw.has("minecraft:"+key))bind(b,BuiltInRegistries.DATA_COMPONENT_TYPE.getValue(Identifier.withDefaultNamespace(key)),raw.get("minecraft:"+key),json);item.builtInRegistryHolder().bindComponents(b.build());}
  List<JsonObject> predicates=new ArrayList<>(List.of(obj(),obj("items","minecraft:stone"),obj("items",arr()),obj("items",arr("minecraft:stone","minecraft:diamond_sword")),obj("items","#minecraft:enchantable/sharp_weapon"),obj("count",0),obj("count",obj("min",2,"max",5)),obj("components",obj("minecraft:custom_name","key")),obj("components",obj("minecraft:custom_name",obj("text","key","bold",true))),part("custom_name",obj()),part("damage",obj()),part("damage",obj("damage",obj("min",3),"durability",obj("min",1,"max",2000))),part("enchantments",arr()),part("enchantments",arr(obj())),part("enchantments",arr(obj("enchantments","minecraft:sharpness","levels",obj("min",2)))),part("stored_enchantments",arr(obj("levels",3))),part("potion_contents","minecraft:healing"),part("custom_data",obj()),part("custom_data",obj("a",1)),part("custom_data",obj("list",arr(obj("x",1)))),part("custom_data",obj("list",arr())),part("container",obj()),part("bundle_contents",obj()),part("firework_explosion",obj("shape","star","has_trail",true)),part("fireworks",obj("flight_duration",obj("min",2))),part("trim",obj("material","minecraft:copper")),part("trim",obj("pattern","minecraft:bolt")),part("villager/variant","minecraft:plains")));
  for(String name:List.of("container","bundle_contents"))for(Object p:List.of(obj("size",0),obj("size",1),obj("contains",arr(obj("items","minecraft:stone"),obj("count",3))),obj("count",arr(obj("test",obj("items","minecraft:stone"),"count",1))),obj("contains",arr()),obj("contains",arr(obj("items","minecraft:stone"),obj("items","minecraft:stone")))))predicates.add(part(name,obj("items",p)));
  predicates.add(part("fireworks",obj("explosions",obj("contains",arr(obj("shape","star","has_trail",true)),"size",1))));
  JsonArray patchInputs=arr(obj(),obj("minecraft:custom_name","key"),obj("minecraft:custom_name",obj("text","key","bold",true)),obj("minecraft:damage",3),obj("!minecraft:damage",obj()),obj("minecraft:enchantments",obj("minecraft:sharpness",3)),obj("minecraft:stored_enchantments",obj("minecraft:sharpness",3)),obj("minecraft:potion_contents",obj("potion","minecraft:healing")),obj("minecraft:custom_data",obj("a",1,"list",arr(obj("x",1,"extra",2),obj("x",2)))),obj("minecraft:container",arr(obj("slot",2,"item",obj("id","minecraft:stone","count",3)))),obj("minecraft:bundle_contents",arr(obj("id","minecraft:stone","count",3))),obj("minecraft:firework_explosion",obj("shape","star","has_trail",true)),obj("minecraft:fireworks",obj("flight_duration",2,"explosions",arr(obj("shape","star","has_trail",true)))),obj("minecraft:trim",obj("material","minecraft:copper","pattern","minecraft:bolt")),obj("minecraft:villager/variant","minecraft:plains"));
  for(String key:List.of("enchantments","stored_enchantments")) {
   predicates.add(obj("components",obj("minecraft:"+key,obj("minecraft:sharpness",3,"minecraft:unbreaking",2))));
   patchInputs.add(obj("minecraft:"+key,obj("minecraft:unbreaking",2,"minecraft:sharpness",3)));
  }
  for(String key:List.of("writable_book_content","written_book_content")) {
   predicates.add(part(key,obj()));
   for(String text:List.of("raw","safe"))predicates.add(part(key,obj("pages",obj("contains",arr(text)))));
  }
  predicates.add(part("written_book_content",obj("title","Title","author","Author","generation",2,"resolved",true)));
  predicates.add(part("written_book_content",obj("title","Safe")));
  predicates.add(part("written_book_content",obj("pages",obj("contains",arr(obj("text","styled","bold",true))))));
  patchInputs.add(obj("minecraft:writable_book_content",obj()));
  patchInputs.add(obj("minecraft:writable_book_content",obj("pages",arr(obj("raw","raw","filtered","safe")))));
  patchInputs.add(obj("minecraft:written_book_content",obj("title","Title","author","Author")));
  patchInputs.add(obj("minecraft:written_book_content",obj("title",obj("raw","Title","filtered","Safe"),"author","Author","generation",2,"resolved",true,"pages",arr(obj("raw","raw","filtered","safe"),obj("text","styled","bold",true)))));
  JsonArray componentHashes=new JsonArray();
  for(var input:patchInputs) {
   var patch=DataComponentPatch.CODEC.parse(json,input).getOrThrow();
   for(var entry:patch.entrySet()) if(entry.getValue().isPresent() && (entry.getKey()==DataComponents.ENCHANTMENTS || entry.getKey()==DataComponents.STORED_ENCHANTMENTS))
    componentHashes.add(componentHash(entry.getKey(),entry.getValue().get(),registries));
  }
  JsonArray states=new JsonArray();List<ItemStack> stacks=new ArrayList<>();
  for(Item item:List.of(Items.STONE,Items.DIAMOND_SWORD,Items.BUNDLE,Items.POTION,Items.FIREWORK_ROCKET))for(int count:new int[]{0,1,3})for(var p:patchInputs){var patch=DataComponentPatch.CODEC.parse(json,p).getOrThrow();var s=new ItemStack(item.builtInRegistryHolder(),count,patch);stacks.add(s);states.add(obj("item",BuiltInRegistries.ITEM.getKey(item).toString(),"count",count,"patch",bytes(DataComponentPatch.CODEC.encodeStart(nbt,patch).getOrThrow())));}
  JsonArray cases=new JsonArray();for(var p:predicates){var predicate=ItemPredicate.CODEC.parse(json,p).getOrThrow();JsonArray results=new JsonArray();for(var s:stacks)results.add(predicate.test(s));cases.add(obj("input",p,"nbt",bytes(ItemPredicate.CODEC.encodeStart(nbt,predicate).getOrThrow()),"matches",results));}
  Files.writeString(Path.of("crates/pumpkin/src/item/item_predicate_cases.json"),G.toJson(obj("states",states,"predicates",cases,"component_hashes",componentHashes)));System.out.println("predicates="+cases.size()+" states="+states.size()+" comparisons="+cases.size()*states.size());
 }
 @SuppressWarnings({"unchecked","rawtypes"})static JsonObject componentHash(DataComponentType type,Object value,RegistryAccess access)throws Exception {
  return obj("component",BuiltInRegistries.DATA_COMPONENT_TYPE.getKey(type).toString(),"nbt",bytes((Tag)type.codec().encodeStart(access.createSerializationContext(NbtOps.INSTANCE),value).getOrThrow()),"hash",((com.google.common.hash.HashCode)type.codec().encodeStart(access.createSerializationContext(net.minecraft.util.HashOps.CRC32C_INSTANCE),value).getOrThrow()).asInt());
 }
 @SuppressWarnings({"unchecked","rawtypes"})static void bind(DataComponentMap.Builder b,DataComponentType type,JsonElement v,RegistryOps<JsonElement> ops){b.set(type,type.codec().parse(ops,v).getOrThrow());}
}
