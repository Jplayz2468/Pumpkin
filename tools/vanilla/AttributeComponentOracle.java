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
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.EquipmentSlotGroup;
import net.minecraft.world.entity.ai.attributes.*;
import net.minecraft.world.item.*;
import net.minecraft.world.item.component.ItemAttributeModifiers;
import net.minecraft.advancements.predicates.ItemPredicate;
public class AttributeComponentOracle {
 static final Gson G=new Gson();
 static JsonObject obj(Object... p){JsonObject o=new JsonObject();for(int i=0;i<p.length;i+=2)o.add((String)p[i],G.toJsonTree(p[i+1]));return o;}
 static JsonArray arr(Object... v){JsonArray a=new JsonArray();for(Object x:v)a.add(G.toJsonTree(x));return a;}
 static JsonArray bytes(Tag tag)throws Exception{var b=new ByteArrayOutputStream();NbtIo.writeAnyTag(tag,new DataOutputStream(b));JsonArray a=new JsonArray();for(byte v:b.toByteArray())a.add(v&255);return a;}
 static JsonArray wire(ItemAttributeModifiers value,RegistryAccess access) {var b=new RegistryFriendlyByteBuf(Unpooled.buffer(),access);ItemAttributeModifiers.STREAM_CODEC.encode(b,value);JsonArray out=new JsonArray();while(b.isReadable())out.add(b.readUnsignedByte());b.release();return out;}
 static void add(JsonArray out,ItemAttributeModifiers value,String prototype,RegistryAccess access,List<ItemPredicate> predicates)throws Exception {
  var nbt=access.createSerializationContext(NbtOps.INSTANCE);var saved=ItemAttributeModifiers.CODEC.encodeStart(nbt,value).getOrThrow();value=ItemAttributeModifiers.CODEC.parse(nbt,saved).getOrThrow();
  var result=obj("nbt",bytes(saved),"wire",wire(value,access),"hash",ItemAttributeModifiers.CODEC.encodeStart(access.createSerializationContext(net.minecraft.util.HashOps.CRC32C_INSTANCE),value).getOrThrow().asInt(),"prototype",prototype);
  var stack=new ItemStack(Items.STICK);stack.set(DataComponents.ATTRIBUTE_MODIFIERS,value);JsonArray matches=new JsonArray();for(var p:predicates)matches.add(p.test(stack));result.add("matches",matches);out.add(result);
 }
 static JsonObject entry(String attr,String id,double amount,String operation){return obj("type",attr,"id",id,"amount",amount,"operation",operation);}
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var access=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);var json=access.createSerializationContext(JsonOps.INSTANCE);var nbt=access.createSerializationContext(NbtOps.INSTANCE);Items.STICK.builtInRegistryHolder().bindComponents(DataComponentMap.EMPTY);
  JsonArray predicateNbt=new JsonArray();List<ItemPredicate> predicates=new ArrayList<>();
  var tests=List.of(obj(),obj("size",0),obj("contains",arr(obj("attribute","minecraft:attack_damage"))),obj("contains",arr(obj("amount",obj("min",0,"max",10)))),obj("contains",arr(obj("amount",2))),obj("contains",arr(obj("operation","add_multiplied_total"))),obj("contains",arr(obj("slot","armor"))),obj("count",arr(obj("test",obj("attribute",arr("minecraft:armor","minecraft:attack_damage")),"count",obj("min",2)))),obj("contains",arr(obj("id","example:test"))));
  for(var collection:tests){var p=ItemPredicate.CODEC.parse(json,obj("predicates",obj("minecraft:attribute_modifiers",obj("modifiers",collection)))).getOrThrow();predicates.add(p);predicateNbt.add(bytes(ItemPredicate.CODEC.encodeStart(nbt,p).getOrThrow()));}
  for(var exact:List.of(arr(),arr(entry("attack_damage","example:test",1.25,"add_value")))){var p=ItemPredicate.CODEC.parse(json,obj("components",obj("minecraft:attribute_modifiers",exact))).getOrThrow();predicates.add(p);predicateNbt.add(bytes(ItemPredicate.CODEC.encodeStart(nbt,p).getOrThrow()));}
  JsonArray cases=new JsonArray();add(cases,ItemAttributeModifiers.EMPTY,"",access,predicates);
  var defaults=JsonParser.parseString(Files.readString(Path.of("assets/items.json"))).getAsJsonObject();for(var item:defaults.entrySet()){var c=item.getValue().getAsJsonObject().getAsJsonObject("components");if(c.has("minecraft:attribute_modifiers"))add(cases,ItemAttributeModifiers.CODEC.parse(json,c.get("minecraft:attribute_modifiers")).getOrThrow(),item.getKey(),access,predicates);}
  for(var attr:BuiltInRegistries.ATTRIBUTE.listElements().toList())add(cases,new ItemAttributeModifiers(List.of(new ItemAttributeModifiers.Entry(attr,new AttributeModifier(Identifier.parse("example:test"),1.25,AttributeModifier.Operation.ADD_VALUE),EquipmentSlotGroup.ANY))),"",access,predicates);
  for(var slot:EquipmentSlotGroup.values())for(var op:AttributeModifier.Operation.values())for(var display:List.of(ItemAttributeModifiers.Display.attributeModifiers(),ItemAttributeModifiers.Display.hidden(),ItemAttributeModifiers.Display.override(Component.literal("Custom attribute").withStyle(net.minecraft.ChatFormatting.BOLD))))add(cases,new ItemAttributeModifiers(List.of(new ItemAttributeModifiers.Entry(Attributes.ATTACK_DAMAGE,new AttributeModifier(Identifier.parse("example:test"),2.5,op),slot,display))),"",access,predicates);
  for(double amount:new double[]{-0.0,0,Double.MIN_VALUE,Double.MAX_VALUE,-2.5,2,Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY,Double.NaN})add(cases,new ItemAttributeModifiers(List.of(new ItemAttributeModifiers.Entry(Attributes.ATTACK_DAMAGE,new AttributeModifier(Identifier.parse("example:test"),amount,AttributeModifier.Operation.ADD_VALUE),EquipmentSlotGroup.ANY))),"",access,predicates);
  var duplicate=List.of(new ItemAttributeModifiers.Entry(Attributes.ARMOR,new AttributeModifier(Identifier.parse("example:test"),2,AttributeModifier.Operation.ADD_VALUE),EquipmentSlotGroup.ARMOR),new ItemAttributeModifiers.Entry(Attributes.ATTACK_DAMAGE,new AttributeModifier(Identifier.parse("example:test"),3,AttributeModifier.Operation.ADD_MULTIPLIED_TOTAL),EquipmentSlotGroup.MAINHAND));add(cases,new ItemAttributeModifiers(duplicate),"",access,predicates);var reversed=new ArrayList<>(duplicate);Collections.reverse(reversed);add(cases,new ItemAttributeModifiers(reversed),"",access,predicates);
  JsonArray codec=new JsonArray();for(var input:List.of(obj(),arr(obj()),arr(entry("missing","test",2,"add_value")),arr(entry("attack_damage","Bad ID",2,"add_value")),arr(entry("attack_damage",":test",2,"add_value")),arr(entry("attack_damage","test",2,"bad")),arr(obj("type","attack_damage","id","test","amount","bad","operation","add_value")),arr(obj("type","attack_damage","id","test","amount",2,"operation","add_value","slot","bad")),arr(obj("type","attack_damage","id","test","amount",2,"operation","add_value","display",obj("type","bad")))))codec.add(obj("nbt",bytes(JsonOps.INSTANCE.convertTo(NbtOps.INSTANCE,input)),"valid",ItemAttributeModifiers.CODEC.parse(json,input).isSuccess()));
  JsonArray network=new JsonArray();for(int op:new int[]{-1,0,1,2,3,Integer.MAX_VALUE})for(int slot:new int[]{-1,0,10,11})for(int display:new int[]{-1,0,1,3}){var buf=new RegistryFriendlyByteBuf(Unpooled.buffer(),access);buf.writeVarInt(1);buf.writeVarInt(0);buf.writeUtf(":test");buf.writeDouble(1.5);buf.writeVarInt(op);buf.writeVarInt(slot);buf.writeVarInt(display);JsonArray input=new JsonArray();for(int i=0;i<buf.writerIndex();i++)input.add(buf.getUnsignedByte(i));var value=ItemAttributeModifiers.STREAM_CODEC.decode(buf);network.add(obj("input",input,"wire",wire(value,access),"nbt",bytes(ItemAttributeModifiers.CODEC.encodeStart(nbt,value).getOrThrow())));buf.release();}
  Files.writeString(Path.of("crates/pumpkin/src/item/attribute_component_cases.json"),G.toJson(obj("cases",cases,"predicates",predicateNbt,"codec",codec,"network",network)));System.out.println("attribute cases="+cases.size()+" predicates="+predicates.size()+" network="+network.size());
 }
}
