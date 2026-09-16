import com.google.gson.*;
import com.mojang.serialization.*;
import java.nio.file.*;
import java.util.*;
import io.netty.buffer.Unpooled;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.component.*;
import net.minecraft.core.registries.*;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.chat.*;
import net.minecraft.resources.*;
import net.minecraft.tags.*;
import net.minecraft.util.RandomSource;
import net.minecraft.util.context.*;
import net.minecraft.world.item.*;
import net.minecraft.world.item.component.*;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.storage.loot.*;

/** Actual loot functions and component network codecs, using canonical instrument registries. */
public class LootNameInstrumentOracle {
 static final Gson G=new Gson();static final Path DATA=Path.of("assets/datapacks/26_2/data/minecraft");
 static JsonObject obj(Object... p){JsonObject o=new JsonObject();for(int i=0;i<p.length;i+=2)o.add((String)p[i],G.toJsonTree(p[i+1]));return o;}
 static JsonArray arr(Object... v){JsonArray a=new JsonArray();for(Object x:v)a.add(G.toJsonTree(x));return a;}
 static JsonObject table(Object... f){return obj("pools",arr(obj("rolls",1,"entries",arr(obj("type","minecraft:dynamic","name","minecraft:input","functions",arr(f))))));}
 static JsonObject name(Object value,String target){JsonObject o=obj("function","minecraft:set_name","target",target);if(value!=null)o.add("name",G.toJsonTree(value));return o;}
 static JsonObject instrument(Object options){return obj("function","minecraft:set_instrument","options",options);}
 static JsonObject count(int n){return obj("function","minecraft:set_count","count",n);}
 static JsonElement text(Component c){return c==null?JsonNull.INSTANCE:ComponentSerialization.CODEC.encodeStart(JsonOps.INSTANCE,c).getOrThrow();}
 static JsonElement instrument(ItemStack s,RegistryOps<JsonElement> ops){var v=s.get(DataComponents.INSTRUMENT);return v==null?JsonNull.INSTANCE:InstrumentComponent.CODEC.encodeStart(ops,v).getOrThrow();}
 static List<Holder<Instrument>> tag(String name,Registry<Instrument> registry)throws Exception{var r=new LinkedHashSet<Holder<Instrument>>();var v=JsonParser.parseString(Files.readString(DATA.resolve("tags/instrument/"+name+".json"))).getAsJsonObject();for(var e:v.getAsJsonArray("values")){String id=e.getAsString();if(id.startsWith("#"))r.addAll(tag(id.substring(1).replace("minecraft:",""),registry));else r.add(registry.get(Identifier.parse(id)).orElseThrow());}return List.copyOf(r);}
 static <T> void wire(JsonArray cases,String component,T value,DataComponentType<T> type,RegistryAccess access,JsonElement json){var buf=new RegistryFriendlyByteBuf(Unpooled.buffer(),access);type.streamCodec().encode(buf,value);byte[] bytes=new byte[buf.readableBytes()];buf.getBytes(0,bytes);JsonArray out=new JsonArray();for(byte b:bytes)out.add(b&255);var entry=obj("component",component,"value",json,"bytes",out,"hash",type.codec().encodeStart(access.createSerializationContext(net.minecraft.util.HashOps.CRC32C_INSTANCE),value).getOrThrow().asInt());if(value instanceof InstrumentComponent c){var v=c.instrument().value();entry.add("playback",obj("sound",net.minecraft.sounds.SoundEvent.CODEC.encodeStart(access.createSerializationContext(JsonOps.INSTANCE),v.soundEvent()).getOrThrow(),"range",v.range(),"duration",net.minecraft.util.Mth.floor(v.useDuration()*20.0F)));}cases.add(entry);buf.release();}
 public static void main(String[] args)throws Exception{
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  var instruments=new MappedRegistry<Instrument>(Registries.INSTRUMENT,Lifecycle.stable());var builtins=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);var baseOps=builtins.createSerializationContext(JsonOps.INSTANCE);
  try(var files=Files.list(DATA.resolve("instrument"))){for(var p:files.sorted().toList())instruments.register(ResourceKey.create(Registries.INSTRUMENT,Identifier.withDefaultNamespace(p.getFileName().toString().replace(".json",""))),Instrument.DIRECT_CODEC.parse(baseOps,JsonParser.parseString(Files.readString(p))).getOrThrow(),RegistrationInfo.BUILT_IN);}
  Map<TagKey<Instrument>,List<Holder<Instrument>>> tags=new HashMap<>();for(String name:List.of("regular_goat_horns","screaming_goat_horns","goat_horns"))tags.put(TagKey.create(Registries.INSTRUMENT,Identifier.withDefaultNamespace(name)),tag(name,instruments));instruments.bindTags(tags);instruments.freeze();List<Registry<?>> regs=new ArrayList<>();BuiltInRegistries.REGISTRY.forEach(regs::add);regs.add(instruments);var access=new RegistryAccess.ImmutableRegistryAccess(regs).freeze();var ops=access.createSerializationContext(JsonOps.INSTANCE);
  var defaults=JsonParser.parseString(Files.readString(Path.of("assets/items.json"))).getAsJsonObject();for(Item item:List.of(Items.STONE,Items.GOAT_HORN)){var v=defaults.getAsJsonObject(BuiltInRegistries.ITEM.getKey(item).getPath()).getAsJsonObject("components");var b=DataComponentMap.builder().set(DataComponents.MAX_STACK_SIZE,v.get("minecraft:max_stack_size").getAsInt()).set(DataComponents.ITEM_NAME,ComponentSerialization.CODEC.parse(ops,v.get("minecraft:item_name")).getOrThrow());if(v.has("minecraft:instrument"))b.set(DataComponents.INSTRUMENT,InstrumentComponent.CODEC.parse(ops,v.get("minecraft:instrument")).getOrThrow());item.builtInRegistryHolder().bindComponents(b.build());}
  Object styled=obj("text","styled","color","red","bold",true,"italic",false,"extra",arr(" plain",obj("text"," suffix","underlined",true)));
  Object translated=obj("translate","example.message","with",arr("arg",obj("text","two","italic",true)),"color","gold");
  Object sequence=arr(obj("text","one","color","blue","extra",arr(" first")),obj("text","two","bold",true));
  JsonArray tables=arr(table(name("café 🐝","custom_name")),table(name(styled,"custom_name")),table(name(translated,"item_name")),table(name(styled,"item_name")),table(name(sequence,"item_name")),table(name(null,"custom_name")),table(name("","item_name")),table(count(0),name(styled,"item_name"),count(1)),table(instrument("#minecraft:regular_goat_horns")),table(instrument("#minecraft:screaming_goat_horns")),table(instrument("#minecraft:goat_horns")),table(instrument(arr("minecraft:sing_goat_horn","minecraft:ponder_goat_horn","minecraft:sing_goat_horn"))),table(instrument(arr())),table(count(0),instrument("#minecraft:goat_horns"),count(1)),table(instrument("#minecraft:goat_horns"),name(translated,"item_name")));
  for(var e:instruments.listElements().toList())tables.add(table(instrument(e.unwrapKey().orElseThrow().identifier().toString())));
  Files.writeString(Path.of(args[0]),G.toJson(tables));List<LootTable> compiled=new ArrayList<>();for(var t:tables)compiled.add(LootTable.DIRECT_CODEC.parse(ops,t).getOrThrow());var ctor=LootContext.class.getDeclaredConstructor(LootParams.class,RandomSource.class,HolderGetter.Provider.class);ctor.setAccessible(true);JsonArray cases=new JsonArray();
  for(int t=0;t<tables.size();t++)for(int kind=0;kind<2;kind++)for(int n=0;n<16;n++){long seed=n*262L;RandomSource random=kind==0?RandomSource.create(seed):new XoroshiroRandomSource(seed);Item item=n%2==0?Items.STONE:Items.GOAT_HORN;ItemStack input=new ItemStack(item,n%3==0?7:1);input.set(DataComponents.CUSTOM_NAME,Component.literal("old"));boolean removed=n%4==0;if(removed){input.remove(DataComponents.ITEM_NAME);input.remove(DataComponents.INSTRUMENT);}LootParams params=new LootParams(null,new ContextMap.Builder().create(new ContextKeySet.Builder().build()),Map.of(Identifier.parse("minecraft:input"),out->out.accept(input.copy())),0);LootContext ctx=ctor.newInstance(params,random,access);JsonArray output=new JsonArray();compiled.get(t).getRandomItemsRaw(ctx,s->output.add(obj("count",s.getCount(),"custom",text(s.get(DataComponents.CUSTOM_NAME)),"name",text(s.get(DataComponents.ITEM_NAME)),"instrument",instrument(s,ops))));cases.add(obj("table",t,"kind",kind,"seed",seed,"item",BuiltInRegistries.ITEM.getKey(item).toString(),"count",input.getCount(),"removed",removed,"output",output,"next",random.nextLong()));}
  Files.writeString(Path.of(args[1]),G.toJson(cases));JsonArray wire=new JsonArray();
  for(Object value:new Object[]{"café 🐝",styled,translated,sequence,""}){var component=ComponentSerialization.CODEC.parse(ops,G.toJsonTree(value)).getOrThrow();wire(wire,"item_name",component,DataComponents.ITEM_NAME,access,text(component));wire(wire,"custom_name",component,DataComponents.CUSTOM_NAME,access,text(component));}
  for(var e:instruments.listElements().toList()){var v=new InstrumentComponent(e);wire(wire,"instrument",v,DataComponents.INSTRUMENT,access,InstrumentComponent.CODEC.encodeStart(ops,v).getOrThrow());}
  for(Object sound:new Object[]{"minecraft:item.goat_horn.sound.7",obj("sound_id","example:horn","range",32.5)}){var json=obj("sound_event",sound,"use_duration",1.25,"range",48.0,"description","custom horn");var v=InstrumentComponent.CODEC.parse(ops,json).getOrThrow();wire(wire,"instrument",v,DataComponents.INSTRUMENT,access,InstrumentComponent.CODEC.encodeStart(ops,v).getOrThrow());}
  Files.writeString(Path.of(args[2]),G.toJson(wire));System.out.println("tables="+tables.size()+" loot cases="+cases.size()+" network cases="+wire.size());
 }
}
