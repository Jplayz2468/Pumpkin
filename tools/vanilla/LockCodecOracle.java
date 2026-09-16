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
import net.minecraft.world.LockCode;
import net.minecraft.world.level.storage.TagValueInput;
import net.minecraft.world.level.storage.TagValueOutput;
import net.minecraft.util.ProblemReporter;
public class LockCodecOracle {
 static final Gson G=new Gson();
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

  List<String> inputs=new ArrayList<>(List.of("{}", "{unknown:1}", "{count:{}}", "{count:{min:2,max:2}}", "{count:{min:5,max:2}}", "{count:2.75d}", "{count:4294967297L}", "{items:':stone'}", "{items:['stone']}", "{items:['stone','stone']}", "{items:[]}", "{items:'missing'}", "{items:'#missing'}", "{items:'#minecraft:planks'}", "{components:{custom_name:'key'}}", "{components:{'minecraft:custom_name':{text:'key',bold:true}}}", "{components:{damage:-1}}", "{components:{damage:2.75d}}", "{predicates:{custom_data:'{a:1}'}}", "{predicates:{custom_data:'bad'}}", "{predicates:{custom_data:'[1]'}}", "{predicates:{custom_data:{a:1}}}"));
  for(String field:List.of("items","count","components","predicates"))for(String value:List.of("'bad'","[]","{}","1","1.2d"))inputs.add("{"+field+":"+value+"}");
  for(String key:List.of("damage","enchantments","stored_enchantments","potion_contents","custom_data","container","bundle_contents","firework_explosion","fireworks","writable_book_content","written_book_content","attribute_modifiers","trim","jukebox_playable","villager/variant","custom_name","missing"))for(String value:List.of("{}","[]","'bad'","1"))inputs.add("{predicates:{'"+key+"':"+value+"}}");
  for(String name:List.of("damage","fireworks","written_book_content"))for(String field:List.of("damage","durability","flight_duration","generation","author","title","resolved"))for(String value:List.of("'bad'","[]","{}","2.5d"))inputs.add("{predicates:{'"+name+"':{"+field+":"+value+"}}}");
  for(String name:List.of("container","bundle_contents","attribute_modifiers","writable_book_content","written_book_content","fireworks")) {
   String field=switch(name){case "attribute_modifiers"->"modifiers";case "writable_book_content","written_book_content"->"pages";case "fireworks"->"explosions";default->"items";};
   for(String value:List.of("{}","{size:{min:5,max:2}}","{contains:[]}","{contains:[{}]}","{count:[{}]}","{count:[{test:{}}]}","{count:[{test:{},count:2}]}","{contains:'bad'}","{size:'bad'}","[]"))inputs.add("{predicates:{'"+name+"':{"+field+":"+value+"}}}");
  }
  for(String field:List.of("attribute","id","amount","operation","slot"))for(String value:List.of("'bad'","{}","[]","2.5d"))inputs.add("{predicates:{attribute_modifiers:{modifiers:{contains:[{"+field+":"+value+"}]}}}}");
  for(String field:List.of("shape","has_trail","has_twinkle"))for(String value:List.of("'bad'","{}","[]","2.5d"))inputs.add("{predicates:{firework_explosion:{"+field+":"+value+"}}}");
  JsonArray cases=new JsonArray();
  for(String input:inputs){var tag=TagParser.parseCompoundFully(input);var result=LockCode.CODEC.parse(nbt,tag);var root=new CompoundTag();root.put("lock",tag);var lock=LockCode.fromTag(TagValueInput.create(ProblemReporter.DISCARDING,registries,root));var saved=TagValueOutput.createWithContext(ProblemReporter.DISCARDING,registries);lock.addToTag(saved);var c=new JsonObject();c.addProperty("input",input);c.add("nbt",bytes(tag));c.addProperty("valid",result.isSuccess());c.add("saved",bytes(saved.buildResult()));var matches=new JsonArray();for(Item item:List.of(Items.AIR,Items.STICK,Items.STONE,Items.OAK_PLANKS,Items.DIAMOND_SWORD))for(int count:new int[]{0,1,3}){var stack=new ItemStack(item,count);stack.set(DataComponents.CUSTOM_NAME,net.minecraft.network.chat.Component.literal("key"));matches.add(lock.unlocksWith(stack));}c.add("matches",matches);cases.add(c);}
  Files.writeString(Path.of("crates/pumpkin/src/item/lock_codec_cases.json"),G.toJson(cases));System.out.println("lock codec cases="+cases.size());
 }
 @SuppressWarnings({"unchecked","rawtypes"})static void bind(DataComponentMap.Builder b,DataComponentType type,JsonElement v,RegistryOps<JsonElement> ops){b.set(type,type.codec().parse(ops,v).getOrThrow());}
}
