// Actual PositionMoveRotation and Entity.calculatePassengerTransition methods.
import java.util.*;
import net.minecraft.world.entity.*;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.level.portal.TeleportTransition;
import sun.misc.Unsafe;
public class TeleportStateOracle {
    static void set(Entity entity, PositionMoveRotation state) throws Exception {
        var p=Entity.class.getDeclaredField("position");p.setAccessible(true);p.set(entity,state.position());
        var y=Entity.class.getDeclaredField("yRot");y.setAccessible(true);y.setFloat(entity,state.yRot());
        var x=Entity.class.getDeclaredField("xRot");x.setAccessible(true);x.setFloat(entity,state.xRot());
    }
    static String state(PositionMoveRotation state) {
        double[] v={state.position().x,state.position().y,state.position().z,state.deltaMovement().x,state.deltaMovement().y,state.deltaMovement().z,state.yRot(),state.xRot()};
        var out=new StringJoiner(",","[","]");for(double x:v)out.add(Long.toUnsignedString(Double.doubleToRawLongBits(x)));return out.toString();
    }
    static PositionMoveRotation random(Random r) {
        return new PositionMoveRotation(new Vec3(r.nextDouble()*80-40,r.nextDouble()*20-10,r.nextDouble()*80-40),new Vec3(r.nextDouble()*4-2,r.nextDouble()*4-2,r.nextDouble()*4-2),r.nextFloat()*720-360,r.nextFloat()*200-100);
    }
    public static void main(String[] args) throws Exception {
        var out=System.out;net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
        var field=Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);var unsafe=(Unsafe)field.get(null);
        var vehicle=(FallDistanceOracle.Probe)unsafe.allocateInstance(FallDistanceOracle.Probe.class);
        var passenger=(FallDistanceOracle.Probe)unsafe.allocateInstance(FallDistanceOracle.Probe.class);
        var method=Entity.class.getDeclaredMethod("calculatePassengerTransition",TeleportTransition.class,Entity.class);method.setAccessible(true);
        var random=new Random(262);out.println("[");
        for(int i=0;i<768;i++) {
            var source=random(random);var change=random(random);var rider=random(random);int flags=i%512;
            var relatives=Relative.unpack(flags);set(vehicle,source);set(passenger,rider);
            var transition=new TeleportTransition(null,change.position(),change.deltaMovement(),change.yRot(),change.xRot(),relatives,TeleportTransition.DO_NOTHING);
            var child=(TeleportTransition)method.invoke(vehicle,transition,passenger);
            out.println((i==0?"":",")+"["+flags+","+state(source)+","+state(change)+","+state(rider)+","+state(PositionMoveRotation.calculateAbsolute(source,change,relatives))+","+state(PositionMoveRotation.of(child))+"]");
        }
        out.println("]");
    }
}
