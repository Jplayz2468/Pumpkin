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
import net.minecraft.util.context.*;
import net.minecraft.world.item.*;
import net.minecraft.world.item.alchemy.*;
import net.minecraft.world.item.component.BlockItemStateProperties;
import net.minecraft.world.level.block.*;
import net.minecraft.world.level.block.state.*;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.storage.loot.*;
import net.minecraft.world.level.storage.loot.parameters.*;
public class LootComponentOracle {
 static Gson gson=new Gson();
 static JsonObject obj(Object... fields){JsonObject o=new JsonObject();for(int i=0;i<fields.length;i+=2)o.add((String)fields[i],gson.toJsonTree(fields[i+1]));return o;}
 static JsonArray arr(Object... values){JsonArray a=new JsonArray();for(Object v:values)a.add(gson.toJsonTree(v));return a;}
 static JsonObject damage(Object value,boolean add){return obj("function","minecraft:set_damage","damage",value,"add",add);}
 static JsonObject potion(String id){return obj("function","minecraft:set_potion","id","minecraft:"+id);}
 static JsonObject copy(String block,String... properties){return obj("function","minecraft:copy_state","block","minecraft:"+block,"properties",properties);}
 static JsonObject table(Object... functions){return obj("pools",arr(obj("rolls",1,"entries",arr(obj("type","minecraft:dynamic","name","minecraft:input","functions",arr(functions))))));}
 static JsonObject uniform(float min,float max){return obj("type","minecraft:uniform","min",min,"max",max);}
 static <T extends Comparable<T>> String value(BlockState state,Property<T> property){return property.getName(state.getValue(property));}
 static JsonObject state(BlockState state){if(state==null)return null;JsonObject values=new JsonObject();for(Property<?> p:state.getProperties())values.addProperty(p.getName(),value(state,p));return obj("block",BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString(),"properties",values);}
 static JsonObject output(ItemStack stack){
  JsonObject result=obj("item",BuiltInRegistries.ITEM.getKey(stack.getItem()).toString(),"count",stack.getCount());
  result.add("damage",gson.toJsonTree(stack.get(DataComponents.DAMAGE)));
  PotionContents pc=stack.get(DataComponents.POTION_CONTENTS);
  result.add("potion",pc==null?JsonNull.INSTANCE:obj("id",pc.potion().map(p->BuiltInRegistries.POTION.getKey(p.value()).toString()).orElse(null),"color",pc.customColor().orElse(null),"name",pc.customName().orElse(null),"effects",pc.customEffects().size()));
  BlockItemStateProperties bs=stack.get(DataComponents.BLOCK_STATE);result.add("state",bs==null?JsonNull.INSTANCE:gson.toJsonTree(bs.properties()));return result;
 }
 public static void main(String[] args)throws Exception{
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  for(Item item:new Item[]{Items.IRON_SWORD,Items.STONE,Items.POTION,Items.AIR})item.builtInRegistryHolder().bindComponents(DataComponentMap.builder().set(DataComponents.MAX_STACK_SIZE,64).build());
  JsonArray tables=arr(table(damage(uniform(0,1),false)),table(damage(uniform(-.5f,.5f),true)),table(damage(.2f,false),damage(.6f,true)),table(obj("function","minecraft:set_count","count",0),damage(uniform(0,1),false),obj("function","minecraft:set_count","count",1)),table(potion("slowness")),table(potion("healing"),potion("poison")),table(copy("beehive","honey_level","facing","missing")),table(copy("wheat","age")),table(copy("oak_stairs","waterlogged","facing")),table(copy("stone","age")),table(obj("function","minecraft:set_count","count",0),potion("healing"),copy("beehive","honey_level"),obj("function","minecraft:set_count","count",1)));
  Files.writeString(Path.of(args[0]),gson.toJson(tables));
  var ctor=LootContext.class.getDeclaredConstructor(LootParams.class,RandomSource.class,HolderGetter.Provider.class);ctor.setAccessible(true);
  Block[] blocks={Blocks.BEEHIVE,Blocks.BEE_NEST,Blocks.WHEAT,Blocks.SUGAR_CANE,Blocks.OAK_STAIRS,Blocks.OAK_SLAB,Blocks.STONE,null};
  JsonArray cases=new JsonArray();
  for(int t=0;t<tables.size();t++)for(int kind=0;kind<2;kind++)for(int n=0;n<32;n++){
   long seed=n==31?-1:n*262L;RandomSource random=kind==0?RandomSource.create(seed):new XoroshiroRandomSource(seed);
   Item item=new Item[]{Items.IRON_SWORD,Items.STONE,Items.POTION}[n%3];ItemStack stack=new ItemStack(item,1);
   Integer max=n%5==0?null:(n%5==1?0:250);Integer initial=n%7==0?null:(n*17-20);boolean unbreakable=n%4==0;
   if(max!=null)stack.set(DataComponents.MAX_DAMAGE,max);if(initial!=null)stack.set(DataComponents.DAMAGE,initial);if(unbreakable)stack.set(DataComponents.UNBREAKABLE,net.minecraft.util.Unit.INSTANCE);
   boolean metadata=n%2==0;
   if(metadata){stack.set(DataComponents.POTION_CONTENTS,new PotionContents(Optional.of(Potions.WATER),Optional.of(123456),List.of(),Optional.of("retained")));stack.set(DataComponents.BLOCK_STATE,new BlockItemStateProperties(Map.of("retained","yes","honey_level","2","age","4")));}
   Block block=blocks[n%blocks.length];BlockState blockState=block==null?null:block.getStateDefinition().getPossibleStates().get((n*7)%block.getStateDefinition().getPossibleStates().size());
   ContextMap.Builder map=new ContextMap.Builder();if(blockState!=null)map.withParameter(LootContextParams.BLOCK_STATE,blockState);
   LootParams params=new LootParams(null,map.create(new ContextKeySet.Builder().optional(LootContextParams.BLOCK_STATE).build()),Map.of(Identifier.parse("minecraft:input"),out->out.accept(stack.copy())),0);
   LootContext context=ctor.newInstance(params,random,null);LootTable table=LootTable.DIRECT_CODEC.parse(JsonOps.INSTANCE,tables.get(t)).getOrThrow();JsonArray results=new JsonArray();table.getRandomItemsRaw(context,out->results.add(output(out)));
   cases.add(obj("table",t,"kind",kind,"seed",seed,"item",BuiltInRegistries.ITEM.getKey(item).toString(),"max",max,"initial",initial,"unbreakable",unbreakable,"metadata",metadata,"context",state(blockState),"output",results,"next",random.nextLong()));
  }
  Files.writeString(Path.of(args[1]),gson.toJson(cases));
 }
}
