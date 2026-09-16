// Calls Entity.checkFallDamage before landing dispatch to verify accumulation width/order.
import java.util.*;
import java.lang.reflect.*;
import net.minecraft.world.entity.*;
import net.minecraft.world.phys.Vec3;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.world.level.storage.*;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.server.level.ServerLevel;
import sun.misc.Unsafe;
public class FallDistanceOracle {
    static class Probe extends Entity {
        boolean water;
        Probe() { super(null,null); }
        public boolean isInWater() { return water; }
        public boolean hurtServer(ServerLevel level, DamageSource source, float damage) { return false; }
        protected void defineSynchedData(SynchedEntityData.Builder builder) {}
        protected void readAdditionalSaveData(ValueInput input) {}
        protected void addAdditionalSaveData(ValueOutput output) {}
    }
    static String bits(double value) { return Long.toUnsignedString(Double.doubleToRawLongBits(value)); }
    public static void main(String[] args) throws Exception {
        var out=System.out; net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var field=Unsafe.class.getDeclaredField("theUnsafe"); field.setAccessible(true);
        var probe=(Probe)((Unsafe)field.get(null)).allocateInstance(Probe.class);
        var method=Entity.class.getDeclaredMethod("checkFallDamage",double.class,boolean.class,BlockState.class,BlockPos.class); method.setAccessible(true);
        Random random=new Random(262); out.println("[");
        for(int i=0;i<1200;i++) {
            if(i%50==0) probe.fallDistance=new double[]{0,16777216.125,1e10,3.00000001}[i/50%4];
            double previous=probe.fallDistance;
            double movement=(random.nextDouble()-.75)*3;
            probe.water=i%7==0;
            method.invoke(probe,movement,false,null,null);
            out.println((i==0?"":",")+"["+bits(previous)+","+bits(movement)+","+probe.water+","+bits(probe.fallDistance)+"]");
        }
        out.println("]");
    }
}
