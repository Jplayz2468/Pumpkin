// Invoke Java 26.2's private EntityFluidInteraction.Tracker on real Entity/Player types.
import java.util.*;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.phys.Vec3;
import net.minecraft.server.level.ServerPlayer;
import sun.misc.Unsafe;
public class FluidCurrentOracle {
    public static void main(String[] args) throws Exception {
        var out=System.out; net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var field=Unsafe.class.getDeclaredField("theUnsafe"); field.setAccessible(true); var unsafe=(Unsafe)field.get(null);
        Entity plain=(Entity)unsafe.allocateInstance(FallDistanceOracle.Probe.class);
        Entity player=(Entity)unsafe.allocateInstance(ServerPlayer.class);
        var type=Class.forName("net.minecraft.world.entity.EntityFluidInteraction$Tracker");
        var constructor=type.getDeclaredConstructor(); constructor.setAccessible(true);
        var accumulate=type.getDeclaredMethod("accumulateCurrent",Vec3.class); accumulate.setAccessible(true);
        var apply=type.getDeclaredMethod("applyCurrentTo",Entity.class,double.class); apply.setAccessible(true);
        Random random=new Random(262); out.println("[");
        for(int i=0;i<1200;i++) {
            boolean isPlayer=i%2==0; var entity=isPlayer?player:plain;
            Vec3 before=new Vec3(i%3==0?0:(random.nextDouble()-.5)*.1,(random.nextDouble()-.5)*8,i%3==0?0:(random.nextDouble()-.5)*.1);
            double scale=new double[]{.014,.007,.0023333333333333335}[i%3];
            var tracker=constructor.newInstance();
            var flows=new StringJoiner(",","[","]");
            for(int j=0;j<i%9;j++) {
                double factor=new double[]{1,.000001,.003,.01,.5}[i%5];
                var flow=new Vec3((random.nextDouble()-.5)*factor,(random.nextDouble()-.5)*factor,(random.nextDouble()-.5)*factor);
                flows.add(BlockClipOracle.bits(flow.x,flow.y,flow.z)); accumulate.invoke(tracker,flow);
            }
            entity.setDeltaMovement(before); apply.invoke(tracker,entity,scale); var after=entity.getDeltaMovement();
            out.println((i==0?"":",")+"["+isPlayer+","+BlockClipOracle.bits(before.x,before.y,before.z)+","+BlockClipOracle.bits(scale)+","+flows+","+BlockClipOracle.bits(after.x,after.y,after.z)+"]");
        }
        out.println("]");
    }
}
