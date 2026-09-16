// Actual PrimedTnt load/save codecs; only synchronized-field storage is substituted.
import java.util.*;
import com.google.gson.Gson;
import net.minecraft.core.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.*;
import net.minecraft.util.ProblemReporter;
import net.minecraft.world.entity.*;
import net.minecraft.world.entity.item.PrimedTnt;
import net.minecraft.world.level.block.*;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.storage.*;
import sun.misc.Unsafe;
public class TntStateOracle {
    static class Probe extends PrimedTnt {
        int fuse;
        BlockState state;
        Probe() { super(EntityTypes.TNT,null); }
        public void setFuse(int value) { fuse=value; }
        public int getFuse() { return fuse; }
        public void setBlockState(BlockState value) { state=value; }
        public BlockState getBlockState() { return state; }
        void read(ValueInput input) { readAdditionalSaveData(input); }
        void write(ValueOutput output) { addAdditionalSaveData(output); }
    }
    public static void main(String[] args) throws Exception {
        var out=System.out;net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
        var field=Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);var unsafe=(Unsafe)field.get(null);
        var probe=(Probe)unsafe.allocateInstance(Probe.class);
        var provider=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
        Random random=new Random(262);Gson json=new Gson();out.println("[");
        for(int i=0;i<612;i++) {
            CompoundTag input=new CompoundTag();
            double number=new double[]{80,0,-1,32768,65537,-1.75,1e12,-1e12}[i%8];
            switch(i%9) {
                case 0 -> input.putByte("fuse",(byte)number);
                case 1 -> input.putShort("fuse",(short)number);
                case 2 -> input.putInt("fuse",(int)number);
                case 3 -> input.putLong("fuse",(long)number);
                case 4 -> input.putFloat("fuse",(float)number);
                case 5 -> input.putDouble("fuse",number);
                case 6 -> input.putString("fuse","invalid");
            }
            double power=new double[]{-5,0,4,7.2,128,129,1e20}[i%7];
            switch(i%8) {
                case 0 -> input.putByte("explosion_power",(byte)power);
                case 1 -> input.putShort("explosion_power",(short)power);
                case 2 -> input.putInt("explosion_power",(int)power);
                case 3 -> input.putLong("explosion_power",(long)power);
                case 4 -> input.putFloat("explosion_power",(float)power);
                case 5 -> input.putDouble("explosion_power",power);
                case 6 -> input.putString("explosion_power","invalid");
            }
            if(i%4!=0) {
                var state=Block.stateById(random.nextInt(Block.BLOCK_STATE_REGISTRY.size()));
                var encoded=(CompoundTag)BlockState.CODEC.encodeStart(NbtOps.INSTANCE,state).getOrThrow();
                if(i%11==0)encoded.putString("Name","minecraft:missing");
                if(i%13==0)encoded.getCompound("Properties").ifPresent(props->{for(String key:new ArrayList<>(props.keySet()))props.putString(key,"invalid");});
                input.put("block_state",encoded);
            }
            if(i%3==0)input.putIntArray("owner",new int[]{1,-2,3,-4});
            if(i%17==0)input.putIntArray("owner",new int[]{1,2,3});
            if(i>=600) {
                var state=new CompoundTag();var props=new CompoundTag();props.putString("unstable","true");state.put("Properties",props);
                switch(i%6) {
                    case 0 -> state.putString("Name","minecraft:missing");
                    case 1 -> state.putString("Name","other:tnt");
                    case 2 -> state.putInt("Name",1);
                    case 3 -> state.putString("Name","tnt");
                    case 4 -> state.putString("Name","minecraft:tnt");
                }
                if(i<606)input.put("block_state",state);else input.putInt("block_state",0);
            }
            probe.read(TagValueInput.create(ProblemReporter.DISCARDING,provider,input));
            var output=TagValueOutput.createWithContext(ProblemReporter.DISCARDING,provider);probe.write(output);
            out.println((i==0?"":",")+"["+json.toJson(input.toString())+","+json.toJson(output.buildResult().toString())+"]");
        }
        out.println("]");
    }
}
