// Actual ValueInput / Vec3 codec plus Entity.load's per-component motion limit.
import java.util.*;
import com.google.gson.Gson;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.*;
import net.minecraft.util.ProblemReporter;
import net.minecraft.world.level.storage.TagValueInput;
import net.minecraft.world.phys.Vec3;
public class SavedMotionOracle {
    public static void main(String[] args) {
        var out=System.out;net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
        var provider=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
        var json=new Gson();var random=new Random(262);out.println("[");
        double[] values={0,-0.0,0.2,-0.04,10,-10,10.000001,-10.000001,1e10,-1e10};
        for(int i=0;i<240;i++) {
            var input=new CompoundTag();var list=new ListTag();
            int count=i%6;
            for(int j=0;j<count;j++) {
                double value=values[random.nextInt(values.length)];
                list.add(switch(i%8) {
                    case 0 -> ByteTag.valueOf((byte)value);
                    case 1 -> ShortTag.valueOf((short)value);
                    case 2 -> IntTag.valueOf((int)value);
                    case 3 -> LongTag.valueOf((long)value);
                    case 4 -> FloatTag.valueOf((float)value);
                    case 5 -> j==1?StringTag.valueOf("bad"):DoubleTag.valueOf(value);
                    default -> DoubleTag.valueOf(value);
                });
            }
            input.put("Motion",switch(i%5) {
                case 0 -> new IntArrayTag(list.stream().filter(t->t instanceof NumericTag).mapToInt(t->((NumericTag)t).intValue()).toArray());
                case 1 -> new LongArrayTag(list.stream().filter(t->t instanceof NumericTag).mapToLong(t->((NumericTag)t).longValue()).toArray());
                default -> list;
            });
            if(i%17==0)input.remove("Motion");
            if(i%19==0)input.putString("Motion","bad");
            var motion=TagValueInput.create(ProblemReporter.DISCARDING,provider,input).read("Motion",Vec3.CODEC).orElse(Vec3.ZERO);
            var bits=new ArrayList<String>();
            for(double value:new double[]{motion.x,motion.y,motion.z})bits.add(Long.toUnsignedString(Double.doubleToRawLongBits(Math.abs(value)>10?0:value)));
            out.println((i==0?"":",")+"["+json.toJson(input.toString())+","+json.toJson(bits)+"]");
        }
        out.println("]");
    }
}
