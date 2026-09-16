import com.google.gson.*;
import java.nio.file.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.ai.attributes.*;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.EquipmentSlotGroup;

/** Exports actual registry ranges and calculated attribute values, without entities. */
public class AttributeValueOracle {
    static String bits(double value) {
        return Long.toUnsignedString(Double.doubleToRawLongBits(value), 16);
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        JsonObject registry = new JsonObject();
        JsonArray cases = new JsonArray();
        for (var holder : BuiltInRegistries.ATTRIBUTE.listElements().toList()) {
            var attribute = (RangedAttribute) holder.value();
            String name = BuiltInRegistries.ATTRIBUTE.getKey(attribute).getPath();
            JsonObject data = new JsonObject();
            data.addProperty("id", BuiltInRegistries.ATTRIBUTE.getId(attribute));
            data.addProperty("default_value", attribute.getDefaultValue());
            data.addProperty("min_value", attribute.getMinValue());
            data.addProperty("max_value", attribute.getMaxValue());
            registry.add(name, data);
            for (double base : new double[] {attribute.getDefaultValue(), attribute.getMinValue(),
                    attribute.getMaxValue(), -0.0, -100, 1.25, Double.MAX_VALUE,
                    Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, Double.NaN}) {
                for (int mode = 0; mode < 4; mode++) {
                    var instance = new AttributeInstance(holder, ignored -> {});
                    instance.setBaseValue(base);
                    JsonArray modifiers = new JsonArray();
                    for (int operation = 0; operation < mode; operation++) {
                        // One modifier per operation: no assumption about hash-map iteration order.
                        double amount = new double[] {2.5, 0.2, -0.3}[operation];
                        String id = "example:operation_" + operation;
                        instance.addTransientModifier(new AttributeModifier(Identifier.parse(id), amount,
                                AttributeModifier.Operation.values()[operation]));
                        JsonObject modifier = new JsonObject();
                        modifier.addProperty("id", id);
                        modifier.addProperty("amount", bits(amount));
                        modifier.addProperty("operation", operation);
                        modifiers.add(modifier);
                    }
                    JsonObject sample = new JsonObject();
                    sample.addProperty("attribute", name);
                    sample.addProperty("base", bits(base));
                    sample.add("modifiers", modifiers);
                    sample.addProperty("value", bits(instance.getValue()));
                    cases.add(sample);
                }
            }
        }
        JsonArray slots = new JsonArray();
        for (var group : EquipmentSlotGroup.values()) {
            for (var slot : EquipmentSlot.values()) {
                JsonObject sample = new JsonObject();
                sample.addProperty("group", group.getSerializedName());
                sample.addProperty("slot", slot.getSerializedName());
                sample.addProperty("matches", group.test(slot));
                slots.add(sample);
            }
        }
        JsonObject fixture = new JsonObject();
        fixture.add("values", cases);
        fixture.add("slots", slots);
        Gson gson = new Gson();
        Files.writeString(Path.of("assets/attributes.json"), gson.toJson(registry));
        Files.writeString(Path.of("crates/pumpkin/src/entity/attribute_value_cases.json"), gson.toJson(fixture));
        System.out.println("attributes=" + registry.size() + " value cases=" + cases.size());
    }
}
