import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import it.unimi.dsi.fastutil.objects.ObjectArrayList;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.util.RandomSource;
import net.minecraft.world.SimpleContainer;
import net.minecraft.world.Container;
import net.minecraft.world.item.*;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.storage.loot.*;
import net.minecraft.world.level.storage.loot.entries.*;
import net.minecraft.world.level.storage.loot.functions.*;
import net.minecraft.world.level.storage.loot.predicates.*;
import net.minecraft.world.level.storage.loot.providers.number.*;
public class LootRandomOracle {
 static JsonArray stack(ItemStack s) {JsonArray a=new JsonArray();a.add(BuiltInRegistries.ITEM.getKey(s.getItem()).toString());a.add(s.getCount());return a;}
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  // These fixture items all have the vanilla 64-item stack limit; bind only the
  // component needed by raw loot/slot splitting, without starting a server.
  for(var item:new Item[]{Items.DIAMOND,Items.COAL,Items.STONE,Items.AIR})
   item.builtInRegistryHolder().bindComponents(net.minecraft.core.component.DataComponentMap.builder().set(net.minecraft.core.component.DataComponents.MAX_STACK_SIZE,64).build());
  var contextCtor=LootContext.class.getDeclaredConstructor(LootParams.class,RandomSource.class,HolderGetter.Provider.class);contextCtor.setAccessible(true);
  Method slots=LootTable.class.getDeclaredMethod("getAvailableSlots",Container.class,RandomSource.class);slots.setAccessible(true);
  Method split=LootTable.class.getDeclaredMethod("shuffleAndSplitItems",ObjectArrayList.class,int.class,RandomSource.class);split.setAccessible(true);
  JsonArray cases=new JsonArray();
  for(int mode=0;mode<4;mode++)for(int kind=0;kind<2;kind++)for(int n=0;n<32;n++) {
   long seed=n==31?-1:n*262L;RandomSource random=kind==0?RandomSource.create(seed):new XoroshiroRandomSource(seed);
   var pool=LootPool.lootPool().setRolls(mode==0?ConstantValue.exactly(1):UniformGenerator.between(2,5));
   pool.add(LootItem.lootTableItem(Items.DIAMOND).setWeight(3).when(LootItemRandomChanceCondition.randomChance(mode==0?1:0.65f)).apply(SetItemCountFunction.setCount(UniformGenerator.between(1,9))));
   if(mode>0)pool.add(LootItem.lootTableItem(Items.COAL).setWeight(1).apply(SetItemCountFunction.setCount(ConstantValue.exactly(2))));
   if(mode==1)pool.add(LootItem.lootTableItem(Items.STICK).setWeight(0).when(LootItemRandomChanceCondition.randomChance(0.2f)));
   if(mode==2)pool.add(EmptyLootItem.emptyItem().setWeight(2).when(LootItemRandomChanceCondition.randomChance(0.5f)));
   if(mode==3)pool.when(LootItemRandomChanceCondition.randomChance(0.25f));
   LootTable table=LootTable.lootTable().withPool(pool).build();
   LootParams params=new LootParams(null,null,Map.of(),0);
   LootContext context=contextCtor.newInstance(params,random,null);
   ObjectArrayList<ItemStack> items=new ObjectArrayList<>();table.getRandomItemsRaw(context,items::add);
   JsonObject c=new JsonObject();c.addProperty("mode",mode);c.addProperty("kind",kind);c.addProperty("seed",seed);
   JsonArray generated=new JsonArray();for(var item:items)generated.add(stack(item));c.add("generated",generated);
   int size=new int[]{0,1,5,9,27}[n%5];SimpleContainer container=new SimpleContainer(size);
   if(size>1&&n%2==0)container.setItem(1,new ItemStack(Items.STONE,1));
   List<Integer> available=(List<Integer>)slots.invoke(table,container,random);
   split.invoke(table,items,available.size(),random);
   for(ItemStack item:items) {if(available.isEmpty())break;container.setItem(available.removeLast(),item);}
   JsonArray filled=new JsonArray();for(int i=0;i<size;i++)filled.add(stack(container.getItem(i)));c.add("filled",filled);c.addProperty("size",size);c.addProperty("occupied",size>1&&n%2==0);c.addProperty("next",random.nextLong());cases.add(c);
  }
  Files.writeString(Path.of(args[0]),new Gson().toJson(cases));
 }
}
