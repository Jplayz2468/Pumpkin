import com.google.gson.*;
import com.mojang.serialization.JsonOps;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.component.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.util.RandomSource;
import net.minecraft.util.context.ContextMap;
import net.minecraft.world.item.*;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.storage.loot.*;
import net.minecraft.world.level.storage.loot.parameters.*;

/** Uses the actual 26.2 codec and evaluator; Rust compiles these same JSON tables. */
public class LootTreeOracle {
 static Gson gson=new Gson();
 static JsonObject obj(Object... fields) {JsonObject o=new JsonObject();for(int i=0;i<fields.length;i+=2)o.add((String)fields[i],gson.toJsonTree(fields[i+1]));return o;}
 static JsonArray arr(Object... values) {JsonArray a=new JsonArray();for(Object v:values)a.add(gson.toJsonTree(v));return a;}
 static JsonObject chance(float p) {return obj("condition","minecraft:random_chance","chance",p);}
 static JsonObject uniform(float a,float b) {return obj("type","minecraft:uniform","min",a,"max",b);}
 static JsonObject binomial(int n,float p) {return obj("type","minecraft:binomial","n",n,"p",p);}
 static JsonObject count(Object n,boolean add) {return obj("function","minecraft:set_count","count",n,"add",add);}
 static JsonObject item(String name,float p) {return obj("type","minecraft:item","name","minecraft:"+name,"conditions",arr(chance(p)));}
 static JsonObject composite(String type,Object... children) {return obj("type","minecraft:"+type,"children",arr(children));}
 static JsonObject pool(Object rolls,Object... entries) {return obj("rolls",rolls,"entries",arr(entries));}
 static JsonObject table(Object... pools) {return obj("pools",arr(pools));}
 static JsonObject nested(JsonObject table) {return obj("type","minecraft:loot_table","value",table);}
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  for(var item:new Item[]{Items.DIAMOND,Items.COAL,Items.STONE,Items.GOLD_INGOT,Items.EMERALD,Items.AIR})
   item.builtInRegistryHolder().bindComponents(DataComponentMap.builder().set(DataComponents.MAX_STACK_SIZE,64).build());
  JsonArray tables=new JsonArray();
  tables.add(table(pool(uniform(2,5),composite("alternatives",item("diamond",.3f),item("coal",.7f),item("gold_ingot",1)))));
  tables.add(table(pool(4,composite("alternatives",composite("sequence",item("diamond",.7f),item("coal",.4f),item("gold_ingot",1)),item("emerald",1)))));
  for(int size=0;size<3;size++) {
   JsonObject group=composite("group");JsonArray children=new JsonArray();for(int i=0;i<size;i++)children.add(item(i==0?"diamond":"coal",.35f));group.add("children",children);
   tables.add(table(pool(5,composite("alternatives",group,item("emerald",1)))));
  }
  JsonObject leaf=item("diamond",1);leaf.add("functions",arr(count(uniform(2,8),false),count(uniform(-1.8f,2.3f),true)));
  JsonObject childPool=pool(binomial(5,.6f),leaf,item("coal",.5f));childPool.add("functions",arr(count(uniform(0,3),true)));
  JsonObject child=table(childPool,pool(2,item("gold_ingot",1)));child.add("functions",arr(count(uniform(0,2),true)));child.addProperty("random_sequence","minecraft:ignored_for_nested_context");
  JsonObject nest=nested(child);nest.add("functions",arr(count(uniform(1,3),true)));
  JsonObject parentPool=pool(2,nest);parentPool.add("functions",arr(obj("function","minecraft:explosion_decay")));
  JsonObject parent=table(parentPool);parent.add("functions",arr(count(1,true)));tables.add(parent);
  JsonObject quality=item("diamond",.65f);quality.addProperty("weight",2);quality.addProperty("quality",3);
  JsonObject other=item("coal",.75f);other.addProperty("weight",4);other.addProperty("quality",-2);
  JsonObject weighted=pool(uniform(1.2f,3.8f),quality,other);weighted.add("bonus_rolls",uniform(.5f,2.0f));
  weighted.add("conditions",arr(obj("condition","minecraft:any_of","terms",arr(chance(.2f),obj("condition","minecraft:inverted","term",chance(.4f))))));tables.add(table(weighted));
  JsonObject counted=item("diamond",1);JsonObject sometimes=count(binomial(12,.45f),true);sometimes.add("conditions",arr(chance(.5f)));
  counted.add("functions",arr(count(300,false),obj("function","minecraft:limit_count","limit",obj("max",uniform(3,10))),sometimes,obj("function","minecraft:explosion_decay")));
  tables.add(table(pool(4,counted)));
  JsonObject dynamic=obj("type","minecraft:dynamic","name","minecraft:sherds","functions",arr(count(uniform(1,8),false)));
  tables.add(table(pool(2,composite("alternatives",dynamic,item("emerald",1)))));
  JsonObject zero=item("diamond",1);zero.addProperty("weight",0);tables.add(table(pool(4,composite("alternatives",zero,item("emerald",1)))));
  JsonObject empty=obj("type","minecraft:empty");tables.add(table(pool(4,composite("alternatives",empty,item("emerald",1)))));
  JsonObject huge=item("diamond",1);huge.add("functions",arr(count(300,false)));tables.add(table(pool(1,huge)));
  Files.writeString(Path.of(args[0]),gson.toJson(tables));
  var ctor=LootContext.class.getDeclaredConstructor(LootParams.class,RandomSource.class,HolderGetter.Provider.class);ctor.setAccessible(true);
  JsonArray cases=new JsonArray();
  for(int t=0;t<tables.size();t++)for(int kind=0;kind<2;kind++)for(int n=0;n<24;n++) {
   LootTable loot=LootTable.DIRECT_CODEC.parse(JsonOps.INSTANCE,tables.get(t)).getOrThrow();
   long seed=n==23?-1:n*262L;float luck=new float[]{-1.25f,0,1.5f,3.0f}[n%4];float radius=new float[]{0,1,2,4}[n%4];
   RandomSource random=kind==0?RandomSource.create(seed):new XoroshiroRandomSource(seed);
   ContextMap.Builder contextMap=new ContextMap.Builder();if(radius>0)contextMap.withParameter(LootContextParams.EXPLOSION_RADIUS,radius);
   final boolean populated=n%2!=0;
   Map<Identifier,LootParams.DynamicDrop> dynamicDrops=Map.of(Identifier.parse("minecraft:sherds"),out->{if(populated){out.accept(new ItemStack(Items.DIAMOND,2));out.accept(new ItemStack(Items.COAL,3));}});
   LootParams params=new LootParams(null,contextMap.create(new net.minecraft.util.context.ContextKeySet.Builder().optional(LootContextParams.EXPLOSION_RADIUS).build()),dynamicDrops,luck);
   LootContext context=ctor.newInstance(params,random,null);JsonArray output=new JsonArray();
   loot.getRandomItemsRaw(context,stack->output.add(arr(BuiltInRegistries.ITEM.getKey(stack.getItem()).toString(),stack.getCount())));
   cases.add(obj("table",t,"kind",kind,"seed",seed,"luck",luck,"radius",radius,"dynamic",n%2!=0,"output",output,"next",random.nextLong()));
  }
  Files.writeString(Path.of(args[1]),gson.toJson(cases));
 }
}
