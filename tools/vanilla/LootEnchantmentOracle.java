import com.google.gson.*;
import com.mojang.serialization.*;
import java.nio.file.*;
import java.util.*;
import java.util.stream.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.core.*;
import net.minecraft.core.component.*;
import net.minecraft.core.registries.*;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.*;
import net.minecraft.tags.*;
import net.minecraft.util.RandomSource;
import net.minecraft.util.Unit;
import net.minecraft.util.context.*;
import net.minecraft.world.item.*;
import net.minecraft.world.item.enchantment.*;
import net.minecraft.world.item.component.*;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.storage.loot.*;
import net.minecraft.world.level.storage.loot.parameters.LootContextParams;
import sun.misc.Unsafe;

/** Actual Java enchantment codecs/functions with canonical definitions and ordered tags. */
public class LootEnchantmentOracle {
 static final Gson GSON=new Gson();
 static final Path DATA=Path.of("assets/datapacks/26_2/data/minecraft");
 static JsonObject obj(Object... pairs){JsonObject o=new JsonObject();for(int i=0;i<pairs.length;i+=2)o.add((String)pairs[i],GSON.toJsonTree(pairs[i+1]));return o;}
 static JsonArray arr(Object... values){JsonArray a=new JsonArray();for(Object v:values)a.add(GSON.toJsonTree(v));return a;}
 static JsonObject table(Object... functions){return obj("pools",arr(obj("rolls",1,"entries",arr(obj("type","minecraft:dynamic","name","minecraft:input","functions",arr(functions))))));}
 static JsonObject count(int n){return obj("function","minecraft:set_count","count",n);}
 static JsonObject uniform(int a,int b){return obj("type","minecraft:uniform","min",a,"max",b);}
 static JsonObject random(Object options,boolean compatible,boolean extra){JsonObject o=obj("function","minecraft:enchant_randomly","only_compatible",compatible,"include_additional_cost_component",extra);if(options!=null)o.add("options",GSON.toJsonTree(options));return o;}
 static JsonObject levels(Object cost,Object options,boolean extra){JsonObject o=obj("function","minecraft:enchant_with_levels","levels",cost,"include_additional_cost_component",extra);if(options!=null)o.add("options",GSON.toJsonTree(options));return o;}
 static JsonObject set(Object values,boolean add){return obj("function","minecraft:set_enchantments","enchantments",values,"add",add);}
 static class Scene extends ServerLevel {
  RegistryAccess registries;
  Scene(){super(null,null,null,null,null,null,false,0,List.of(),false);}
  @Override public RegistryAccess registryAccess(){return registries;}
 }
 static <T> List<Holder<T>> tag(String folder,String name,Registry<T> registry)throws Exception {
  var result=new LinkedHashSet<Holder<T>>();
  var data=JsonParser.parseString(Files.readString(DATA.resolve("tags/"+folder+"/"+name+".json"))).getAsJsonObject();
  for(var entry:data.getAsJsonArray("values")){
   String id=entry.isJsonObject()?entry.getAsJsonObject().get("id").getAsString():entry.getAsString();
   if(id.startsWith("#"))result.addAll(tag(folder,id.substring(1).replace("minecraft:",""),registry));
   else result.add(registry.get(Identifier.parse(id)).orElseThrow());
  }
  return List.copyOf(result);
 }
 static <T> Map<TagKey<T>,List<Holder<T>>> tags(String folder,Registry<T> registry)throws Exception {
  Map<TagKey<T>,List<Holder<T>>> result=new HashMap<>();Path root=DATA.resolve("tags/"+folder);
  try(var files=Files.walk(root)){for(var path:files.filter(p->p.toString().endsWith(".json")).sorted().toList()){
   String name=root.relativize(path).toString().replace(".json","");
   result.put(TagKey.create(registry.key(),Identifier.withDefaultNamespace(name)),tag(folder,name,registry));
  }}return result;
 }
 static JsonObject enchantments(ItemEnchantments values){if(values==null)return null;JsonObject o=new JsonObject();for(var e:values.entrySet())o.addProperty(e.getKey().unwrapKey().orElseThrow().identifier().toString(),e.getIntValue());return o;}
 static JsonObject output(ItemStack stack){return obj("item",BuiltInRegistries.ITEM.getKey(stack.getItem()).toString(),"count",stack.getCount(),"name",stack.get(DataComponents.CUSTOM_NAME)==null?null:stack.get(DataComponents.CUSTOM_NAME).getString(),"enchantments",enchantments(stack.get(DataComponents.ENCHANTMENTS)),"stored",enchantments(stack.get(DataComponents.STORED_ENCHANTMENTS)),"extra",stack.get(DataComponents.ADDITIONAL_TRADE_COST));}
 static ItemStack stack(Item item,int mode){ItemStack s=new ItemStack(item,mode==4?7:1);if(mode==1||mode==4){s.set(DataComponents.CUSTOM_NAME,Component.literal("input"));s.set(DataComponents.ADDITIONAL_TRADE_COST,42);}if(mode==2){s.remove(DataComponents.ENCHANTMENTS);s.remove(DataComponents.STORED_ENCHANTMENTS);}if(mode==3)s.remove(DataComponents.ENCHANTABLE);return s;}
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  var items=JsonParser.parseString(Files.readString(Path.of("assets/items.json"))).getAsJsonObject();
  for(Item item:BuiltInRegistries.ITEM){var source=items.getAsJsonObject(BuiltInRegistries.ITEM.getKey(item).getPath()).getAsJsonObject("components");var b=DataComponentMap.builder().set(DataComponents.MAX_STACK_SIZE,source.get("minecraft:max_stack_size").getAsInt());if(source.has("minecraft:enchantable"))b.set(DataComponents.ENCHANTABLE,new Enchantable(source.getAsJsonObject("minecraft:enchantable").get("value").getAsInt()));if(source.has("minecraft:enchantments"))b.set(DataComponents.ENCHANTMENTS,ItemEnchantments.EMPTY);if(source.has("minecraft:stored_enchantments"))b.set(DataComponents.STORED_ENCHANTMENTS,ItemEnchantments.EMPTY);item.builtInRegistryHolder().bindComponents(b.build());}
  BuiltInRegistries.ITEM.prepareTagReload(new TagLoader.LoadResult<>(Registries.ITEM,tags("item",BuiltInRegistries.ITEM))).apply();
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
  try(var files=Files.list(DATA.resolve("enchantment"))){for(var p:files.sorted().toList()){var value=Enchantment.DIRECT_CODEC.parse(ops,JsonParser.parseString(Files.readString(p))).getOrThrow();enchantments.register(ResourceKey.create(Registries.ENCHANTMENT,Identifier.withDefaultNamespace(p.getFileName().toString().replace(".json",""))),value,RegistrationInfo.BUILT_IN);}}
  enchantments.bindTags(tags("enchantment",enchantments));enchantments.freeze();
  List<Registry<?>> all=new ArrayList<>();BuiltInRegistries.REGISTRY.forEach(all::add);all.add(enchantments);var registries=new RegistryAccess.ImmutableRegistryAccess(all).freeze();
  var field=Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);var unsafe=(Unsafe)field.get(null);Scene scene=(Scene)unsafe.allocateInstance(Scene.class);scene.registries=registries;
  var ctor=LootContext.class.getDeclaredConstructor(LootParams.class,RandomSource.class,HolderGetter.Provider.class);ctor.setAccessible(true);
  Object onTable="#minecraft:in_enchanting_table";
  JsonArray tables=arr(table(random(null,true,false)),table(random(onTable,true,true)),table(random(arr("minecraft:sharpness","minecraft:unbreaking","minecraft:sharpness"),false,true)),table(random(arr(),true,true)),table(random("minecraft:mending",true,true)),table(levels(uniform(1,50),null,true)),table(levels(30,onTable,true)),table(levels(0,arr(),true)),table(levels(30,arr("minecraft:thorns","minecraft:sharpness","minecraft:efficiency"),false)),table(set(obj("minecraft:sharpness",3),false)),table(set(obj("minecraft:sharpness",3),false),set(obj("minecraft:sharpness",uniform(-5,260)),true)),table(set(obj("minecraft:mending",1,"minecraft:sharpness",300),false),set(obj("minecraft:sharpness",0),false)),table(set(obj(),false)),table(count(0),random(null,false,true),count(1)),table(count(0),levels(uniform(1,50),null,true),count(1)),table(count(0),set(obj("minecraft:sharpness",uniform(1,5)),true),count(1)),table(count(130),set(obj("minecraft:sharpness",2),false)),table(count(130),random(null,true,false)),table(random("minecraft:sharpness",false,false),random("minecraft:sharpness",false,false)),table(levels(30,onTable,false),levels(30,onTable,false)));
  Files.writeString(Path.of(args[0]),GSON.toJson(tables));List<LootTable> compiled=new ArrayList<>();var codecOps=registries.createSerializationContext(JsonOps.INSTANCE);for(var json:tables)compiled.add(LootTable.DIRECT_CODEC.parse(codecOps,json).getOrThrow());
  JsonArray cases=new JsonArray();Item[] targets={Items.BOOK,Items.ENCHANTED_BOOK,Items.DIAMOND_SWORD,Items.IRON_AXE,Items.DIAMOND_CHESTPLATE,Items.DIAMOND_BOOTS,Items.BOW,Items.CROSSBOW,Items.MACE,Items.SHEARS,Items.STONE};
  for(int t=0;t<tables.size();t++)for(int kind=0;kind<2;kind++)for(int n=0;n<55;n++){
   Item item=targets[n%targets.length];int mode=n/targets.length;long seed=n*262L;RandomSource random=kind==0?RandomSource.create(seed):new XoroshiroRandomSource(seed);ItemStack input=stack(item,mode);boolean extra=n%2==0;
   var builder=new ContextMap.Builder();var keys=new ContextKeySet.Builder();if(extra){builder.withParameter(LootContextParams.ADDITIONAL_COST_COMPONENT_ALLOWED,Unit.INSTANCE);keys.required(LootContextParams.ADDITIONAL_COST_COMPONENT_ALLOWED);}
   LootParams params=new LootParams(scene,builder.create(keys.build()),Map.of(Identifier.parse("minecraft:input"),out->out.accept(input.copy())),0);LootContext ctx=ctor.newInstance(params,random,registries);JsonArray outputs=new JsonArray();compiled.get(t).getRandomItemsRaw(ctx,out->outputs.add(output(out)));cases.add(obj("table",t,"kind",kind,"seed",seed,"item",BuiltInRegistries.ITEM.getKey(item).toString(),"mode",mode,"extra",extra,"output",outputs,"next",random.nextLong()));
  }
  Files.writeString(Path.of(args[1]),GSON.toJson(cases));
  JsonArray selection=new JsonArray();
  for(Item item:BuiltInRegistries.ITEM)for(int kind=0;kind<2;kind++)for(int cost:new int[]{1,15,30,50}){
   long seed=BuiltInRegistries.ITEM.getId(item)*262L+cost;RandomSource random=kind==0?RandomSource.create(seed):new XoroshiroRandomSource(seed);ItemStack input=stack(item,0);JsonArray costs=new JsonArray();for(int slot=0;slot<3;slot++)costs.add(EnchantmentHelper.getEnchantmentCost(random,slot,cost,input));
   var chosen=EnchantmentHelper.selectEnchantment(random,input,cost,enchantments.getOrThrow(EnchantmentTags.IN_ENCHANTING_TABLE).stream());if(input.is(Items.BOOK)&&chosen.size()>1)chosen.remove(random.nextInt(chosen.size()));JsonArray selected=new JsonArray();for(var e:chosen)selected.add(obj("id",e.enchantment().unwrapKey().orElseThrow().identifier().toString(),"level",e.level()));
   selection.add(obj("item",BuiltInRegistries.ITEM.getKey(item).toString(),"kind",kind,"cost",cost,"seed",seed,"costs",costs,"selected",selected,"next",random.nextLong()));
  }
  Files.writeString(Path.of(args[2]),GSON.toJson(selection));System.out.println("enchantments="+enchantments.size()+" tables="+tables.size()+" loot cases="+cases.size()+" selection cases="+selection.size());
 }
}
