import com.google.gson.*;
import com.mojang.serialization.JsonOps;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.component.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.util.RandomSource;
import net.minecraft.util.context.*;
import net.minecraft.world.item.*;
import net.minecraft.world.item.component.ItemContainerContents;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.entity.ChestBlockEntity;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.storage.loot.*;
import net.minecraft.world.level.storage.loot.parameters.*;

/** Real 26.2 codec/evaluator; contexts are constructed without a running ServerLevel. */
public class LootCopyComponentsOracle {
 static Gson gson=new Gson();
 static JsonObject obj(Object... fields){JsonObject o=new JsonObject();for(int i=0;i<fields.length;i+=2)o.add((String)fields[i],gson.toJsonTree(fields[i+1]));return o;}
 static JsonArray arr(Object... values){JsonArray a=new JsonArray();for(Object v:values)a.add(gson.toJsonTree(v));return a;}
 static JsonObject copy(String source,Object include,Object exclude){JsonObject f=obj("function","minecraft:copy_components","source",source);if(include!=null)f.add("include",gson.toJsonTree(include));if(exclude!=null)f.add("exclude",gson.toJsonTree(exclude));return f;}
 static JsonObject table(Object... functions){return obj("pools",arr(obj("rolls",1,"entries",arr(obj("type","minecraft:dynamic","name","minecraft:input","functions",arr(functions))))));}
 static JsonObject output(ItemStack stack){JsonArray items=new JsonArray();ItemContainerContents container=stack.get(DataComponents.CONTAINER);if(container!=null)container.nonEmptyItemCopyStream().forEach(i->items.add(obj("item",BuiltInRegistries.ITEM.getKey(i.getItem()).toString(),"count",i.getCount())));return obj("count",stack.getCount(),"name",stack.get(DataComponents.CUSTOM_NAME)==null?null:stack.get(DataComponents.CUSTOM_NAME).getString(),"damage",stack.get(DataComponents.DAMAGE),"max",stack.get(DataComponents.MAX_STACK_SIZE),"container",container==null?null:items);}
 public static void main(String[] args)throws Exception{
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  for(Item item:new Item[]{Items.STONE,Items.DIAMOND})item.builtInRegistryHolder().bindComponents(DataComponentMap.builder().set(DataComponents.MAX_STACK_SIZE,64).build());
  JsonObject conditional=copy("tool",null,null);conditional.add("conditions",arr(obj("condition","minecraft:random_chance","chance",.5)));
  JsonArray tables=arr(table(copy("tool",null,null)),table(copy("tool",arr(),null)),table(copy("tool",arr("minecraft:custom_name","minecraft:damage"),arr("minecraft:custom_name"))),table(copy("tool",null,arr("minecraft:damage","minecraft:max_stack_size"))),table(copy("block_entity",null,null)),table(copy("block_entity",arr("minecraft:custom_name"),null)),table(copy("tool",null,null),copy("block_entity",null,arr("minecraft:custom_name"))),table(conditional),table(obj("function","minecraft:set_count","count",0),copy("tool",null,null),obj("function","minecraft:set_count","count",1)));
  Files.writeString(Path.of(args[0]),gson.toJson(tables));
  var ctor=LootContext.class.getDeclaredConstructor(LootParams.class,RandomSource.class,HolderGetter.Provider.class);ctor.setAccessible(true);
  JsonArray cases=new JsonArray();
  for(int t=0;t<tables.size();t++)for(int kind=0;kind<2;kind++)for(int n=0;n<16;n++){
   long seed=n*262L;RandomSource random=kind==0?RandomSource.create(seed):new XoroshiroRandomSource(seed);
   ItemStack target=new ItemStack(Items.STONE);target.set(DataComponents.CUSTOM_NAME,Component.literal("retained"));target.set(DataComponents.DAMAGE,5);
   ItemStack tool=new ItemStack(Items.STONE,n%7==0?0:1);if(n%2==0)tool.set(DataComponents.CUSTOM_NAME,Component.literal("source"));if(n%4==0)tool.set(DataComponents.DAMAGE,n);if(n%5==0)tool.remove(DataComponents.MAX_STACK_SIZE);
   ChestBlockEntity chest=new ChestBlockEntity(BlockPos.ZERO,Blocks.CHEST.defaultBlockState());
   var components=DataComponentMap.builder().set(DataComponents.CUSTOM_NAME,Component.literal("chest")).set(DataComponents.DAMAGE,37).build();chest.setComponents(components);
   chest.setItem(2,new ItemStack(Items.DIAMOND,3));
   // Apply the name through the implicit component API as well: it overrides stored values.
   chest.applyComponents(DataComponentMap.EMPTY,DataComponentPatch.builder().set(DataComponents.CUSTOM_NAME,Component.literal("chest")).set(DataComponents.CONTAINER,ItemContainerContents.fromItems(List.of(ItemStack.EMPTY,ItemStack.EMPTY,new ItemStack(Items.DIAMOND,3)))).set(DataComponents.DAMAGE,37).build());
   ContextMap.Builder map=new ContextMap.Builder();if(n%8!=0)map.withParameter(LootContextParams.TOOL,tool);if(n%3!=0)map.withParameter(LootContextParams.BLOCK_ENTITY,chest);
   LootParams params=new LootParams(null,map.create(new ContextKeySet.Builder().optional(LootContextParams.TOOL).optional(LootContextParams.BLOCK_ENTITY).build()),Map.of(Identifier.parse("minecraft:input"),out->out.accept(target.copy())),0);
   LootContext context=ctor.newInstance(params,random,null);LootTable table=LootTable.DIRECT_CODEC.parse(JsonOps.INSTANCE,tables.get(t)).getOrThrow();JsonArray results=new JsonArray();table.getRandomItemsRaw(context,out->results.add(output(out)));
   cases.add(obj("table",t,"kind",kind,"seed",seed,"n",n,"output",results,"next",random.nextLong()));
  }
  Files.writeString(Path.of(args[1]),gson.toJson(cases));
 }
}
