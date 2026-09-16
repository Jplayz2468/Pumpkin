import com.google.gson.*;
import com.mojang.serialization.JsonOps;
import java.nio.file.*;
import java.util.*;
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
import net.minecraft.util.context.*;
import net.minecraft.world.effect.*;
import net.minecraft.world.item.*;
import net.minecraft.world.item.component.*;
import net.minecraft.world.item.crafting.*;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.storage.loot.*;
import sun.misc.Unsafe;

/** Real Java functions and recipe manager; the level supplies only recipeAccess. */
public class LootTransformOracle {
 static final Gson GSON = new Gson();
 static final Path DATA = Path.of("assets/datapacks/26_2/data/minecraft");
 static JsonObject obj(Object... pairs) { JsonObject o=new JsonObject(); for(int i=0;i<pairs.length;i+=2)o.add((String)pairs[i],GSON.toJsonTree(pairs[i+1]));return o; }
 static JsonArray arr(Object... values) { JsonArray a=new JsonArray();for(Object v:values)a.add(GSON.toJsonTree(v));return a; }
 static JsonObject table(Object... functions) { return obj("pools",arr(obj("rolls",1,"entries",arr(obj("type","minecraft:dynamic","name","minecraft:input","functions",arr(functions)))))); }
 static JsonObject uniform(float min,float max) { return obj("type","minecraft:uniform","min",min,"max",max); }
 static JsonObject count(int n) { return obj("function","minecraft:set_count","count",n); }
 static JsonObject smelt(boolean useInput) { return obj("function","minecraft:furnace_smelt","use_input_count",useInput); }
 static JsonObject amplifier(Object n) { return obj("function","minecraft:set_ominous_bottle_amplifier","amplifier",n); }
 static JsonObject effect(String id,Object duration) { return obj("type",id,"duration",duration); }
 static JsonObject stew(Object... effects) { return obj("function","minecraft:set_stew_effect","effects",arr(effects)); }
 static class RecipeLevel extends ServerLevel {
  RecipeManager recipes;
  RecipeLevel() { super(null,null,null,null,null,null,false,0,List.of(),false); }
  @Override public RecipeManager recipeAccess() { return recipes; }
 }
 static List<Holder<Item>> tag(String name) throws Exception {
  var result=new LinkedHashSet<Holder<Item>>();
  var data=JsonParser.parseString(Files.readString(DATA.resolve("tags/item/"+name+".json"))).getAsJsonObject();
  for(var entry:data.getAsJsonArray("values")) {
   String id=entry.isJsonObject()?entry.getAsJsonObject().get("id").getAsString():entry.getAsString();
   if(id.startsWith("#"))result.addAll(tag(id.substring(1).replace("minecraft:","")));
   else result.add(BuiltInRegistries.ITEM.getValue(Identifier.parse(id)).builtInRegistryHolder());
  }
  return List.copyOf(result);
 }
 static JsonObject output(ItemStack stack) {
  JsonArray effects=null;var stew=stack.get(DataComponents.SUSPICIOUS_STEW_EFFECTS);
  if(stew!=null){effects=new JsonArray();for(var effect:stew.effects())effects.add(obj("id",BuiltInRegistries.MOB_EFFECT.getKey(effect.effect().value()).toString(),"duration",effect.duration()));}
  var amp=stack.get(DataComponents.OMINOUS_BOTTLE_AMPLIFIER);
  return obj("item",BuiltInRegistries.ITEM.getKey(stack.getItem()).toString(),"count",stack.getCount(),"name",stack.get(DataComponents.CUSTOM_NAME)==null?null:stack.get(DataComponents.CUSTOM_NAME).getString(),"damage",stack.get(DataComponents.DAMAGE),"amplifier",amp==null?null:amp.value(),"stew",effects);
 }
 public static void main(String[] args) throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  // Explicitly bind the component defaults being compared from the canonical item export.
  var items=JsonParser.parseString(Files.readString(Path.of("assets/items.json"))).getAsJsonObject();
  for(Item item:BuiltInRegistries.ITEM) {
   String name=BuiltInRegistries.ITEM.getKey(item).getPath();
   var source=items.getAsJsonObject(name).getAsJsonObject("components");
   var builder=DataComponentMap.builder().set(DataComponents.MAX_STACK_SIZE,source.get("minecraft:max_stack_size").getAsInt());
   if(source.has("minecraft:suspicious_stew_effects"))builder.set(DataComponents.SUSPICIOUS_STEW_EFFECTS,SuspiciousStewEffects.EMPTY);
   if(source.has("minecraft:ominous_bottle_amplifier"))builder.set(DataComponents.OMINOUS_BOTTLE_AMPLIFIER,new OminousBottleAmplifier(source.get("minecraft:ominous_bottle_amplifier").getAsInt()));
   item.builtInRegistryHolder().bindComponents(builder.build());
  }
  Map<TagKey<Item>,List<Holder<Item>>> tags=new HashMap<>();
  for(String name:List.of("smelts_to_glass","logs_that_burn","leaves"))tags.put(TagKey.create(Registries.ITEM,Identifier.withDefaultNamespace(name)),tag(name));
  BuiltInRegistries.ITEM.prepareTagReload(new TagLoader.LoadResult<>(Registries.ITEM,tags)).apply();
  var registries=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
  var ops=registries.createSerializationContext(JsonOps.INSTANCE);
  List<RecipeHolder<?>> recipes=new ArrayList<>();
  try(var files=Files.list(DATA.resolve("recipe"))) {
   for(Path path:files.filter(p->Files.isRegularFile(p)&&p.toString().endsWith(".json")).sorted().toList()) {
    var data=JsonParser.parseString(Files.readString(path)).getAsJsonObject();
    if(!data.get("type").getAsString().equals("minecraft:smelting"))continue;
    var recipe=Recipe.CODEC.parse(ops,data).getOrThrow();
    recipes.add(new RecipeHolder<>(ResourceKey.create(Registries.RECIPE,Identifier.withDefaultNamespace(path.getFileName().toString().replace(".json",""))),recipe));
   }
  }
  var manager=new RecipeManager(registries);var recipeField=RecipeManager.class.getDeclaredField("recipes");recipeField.setAccessible(true);recipeField.set(manager,RecipeMap.create(recipes));
  var field=Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);var unsafe=(Unsafe)field.get(null);
  RecipeLevel level=(RecipeLevel)unsafe.allocateInstance(RecipeLevel.class);level.recipes=manager;
  var contextCtor=LootContext.class.getDeclaredConstructor(LootParams.class,RandomSource.class,HolderGetter.Provider.class);contextCtor.setAccessible(true);
  var choice=stew(effect("minecraft:poison",uniform(1,4)),effect("minecraft:saturation",uniform(1,4)),effect("minecraft:instant_health",2),effect("minecraft:instant_damage",3));
  JsonArray tables=arr(table(smelt(true)),table(smelt(false)),table(count(128),smelt(true)),table(count(0),smelt(true),count(1)),table(amplifier(uniform(-3,8))),table(amplifier(3.5f)),table(count(0),amplifier(uniform(-3,8)),count(1)),table(choice),table(choice,choice),table(stew()),table(count(0),choice,count(1)),table(amplifier(4),smelt(true)),table(smelt(true),amplifier(uniform(0,4))),table(count(-1),smelt(true)));
  // Exercise every registered effect, including tick conversion and integer overflow.
  for(MobEffect effect:BuiltInRegistries.MOB_EFFECT)tables.add(table(stew(effect(BuiltInRegistries.MOB_EFFECT.getKey(effect).toString(),107374184f))));
  Files.writeString(Path.of(args[0]),GSON.toJson(tables));
  List<LootTable> compiled=new ArrayList<>();for(var json:tables)compiled.add(LootTable.DIRECT_CODEC.parse(ops,json).getOrThrow());
  JsonArray cases=new JsonArray();
  for(int t=0;t<tables.size();t++)for(int kind=0;kind<2;kind++)for(int n=0;n<16;n++) {
   Item item=t>=14?Items.SUSPICIOUS_STEW:new Item[]{Items.BEEF,Items.DIRT,Items.SUSPICIOUS_STEW,Items.OMINOUS_BOTTLE}[n%4];
   run(cases,compiled,contextCtor,level,registries,t,kind,n*262L,item,n%3==0?65:1,n%2==0);
  }
  // Exhaustive built-in input mapping against Java's actual 73-recipe manager.
  for(Item item:BuiltInRegistries.ITEM)run(cases,compiled,contextCtor,level,registries,0,0,0,item,65,true);
  Files.writeString(Path.of(args[1]),GSON.toJson(cases));
  System.out.println("tables="+tables.size()+" cases="+cases.size()+" smelting recipes="+recipes.size());
 }
 static void run(JsonArray cases,List<LootTable> tables,java.lang.reflect.Constructor<LootContext> ctor,RecipeLevel level,HolderGetter.Provider registries,int table,int kind,long seed,Item item,int count,boolean metadata) throws Exception {
  RandomSource random=kind==0?RandomSource.create(seed):new XoroshiroRandomSource(seed);
  ItemStack stack=new ItemStack(item,count);
  if(metadata) {
   stack.set(DataComponents.CUSTOM_NAME,Component.literal("input"));stack.set(DataComponents.DAMAGE,17);stack.set(DataComponents.OMINOUS_BOTTLE_AMPLIFIER,new OminousBottleAmplifier(2));
   stack.set(DataComponents.SUSPICIOUS_STEW_EFFECTS,new SuspiciousStewEffects(List.of(new SuspiciousStewEffects.Entry(MobEffects.POISON,13))));
  }
  LootParams params=new LootParams(level,new ContextMap.Builder().create(new ContextKeySet.Builder().build()),Map.of(Identifier.parse("minecraft:input"),out->out.accept(stack.copy())),0);
  LootContext context=ctor.newInstance(params,random,registries);JsonArray outputs=new JsonArray();tables.get(table).getRandomItemsRaw(context,out->outputs.add(output(out)));
  cases.add(obj("table",table,"kind",kind,"seed",seed,"item",BuiltInRegistries.ITEM.getKey(item).toString(),"count",count,"metadata",metadata,"output",outputs,"next",random.nextLong()));
 }
}
