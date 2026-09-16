import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.properties.Property;
/** Stable groups according to the actual Java Property.equals implementations. */
public class BlockPropertyIdentityOracle {
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  List<Block> blocks=new ArrayList<>();BuiltInRegistries.BLOCK.forEach(blocks::add);
  blocks.sort(Comparator.comparing(b->BuiltInRegistries.BLOCK.getKey(b).toString()));
  List<Property<?>> identities=new ArrayList<>();JsonObject result=new JsonObject();
  for(Block block:blocks) {
   List<Property<?>> properties=new ArrayList<>(block.getStateDefinition().getProperties());properties.sort(Comparator.comparing(Property::getName));
   JsonObject ids=new JsonObject();
   for(Property<?> property:properties) {int id=identities.indexOf(property);if(id<0){id=identities.size();identities.add(property);}ids.addProperty(property.getName(),id);}
   result.add(BuiltInRegistries.BLOCK.getKey(block).toString(),ids);
  }
  Files.writeString(Path.of(args[0]),new GsonBuilder().setPrettyPrinting().create().toJson(result)+"\n");
 }
}
