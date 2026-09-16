// Invokes Projectile.hitTargetOrDeflectSelf on an actual Arrow for border impacts.
import java.util.*;
import net.minecraft.core.*;
import net.minecraft.util.RandomSource;
import net.minecraft.world.entity.*;
import net.minecraft.world.entity.projectile.*;
import net.minecraft.world.entity.projectile.arrow.Arrow;
import net.minecraft.world.phys.*;
import sun.misc.Unsafe;
public class BorderDeflectionOracle {
    public static void main(String[] args) throws Exception {
        var out=System.out;net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
        var field=Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);var unsafe=(Unsafe)field.get(null);
        var arrow=(Arrow)unsafe.allocateInstance(Arrow.class);
        FluidInteractionOracle.set(Entity.class,arrow,"level",unsafe.allocateInstance(FluidInteractionOracle.Scene.class));
        var method=Projectile.class.getDeclaredMethod("hitTargetOrDeflectSelf",HitResult.class);method.setAccessible(true);
        Random random=new Random(262);out.println("[");
        for(int i=0;i<200;i++) {
            long seed=random.nextLong();var rng=RandomSource.create(seed);
            FluidInteractionOracle.set(Entity.class,arrow,"random",rng);
            Vec3 motion=new Vec3(random.nextDouble()*10-5,random.nextDouble()*10-5,random.nextDouble()*10-5);
            float yaw=(random.nextFloat()-.5F)*720;
            arrow.setDeltaMovement(motion);arrow.setYRot(yaw);arrow.yRotO=yaw;arrow.needsSync=false;
            method.invoke(arrow,new BlockHitResult(Vec3.ZERO,Direction.EAST,BlockPos.ZERO,false,true));
            var velocity=arrow.getDeltaMovement();
            out.println((i==0?"":",")+"["+seed+","+BlockClipOracle.bits(motion.x,motion.y,motion.z)+","+Integer.toUnsignedString(Float.floatToRawIntBits(yaw))+","+BlockClipOracle.bits(velocity.x,velocity.y,velocity.z)+","+Integer.toUnsignedString(Float.floatToRawIntBits(arrow.getYRot()))+","+arrow.needsSync+","+rng.nextLong()+"]");
        }
        out.println("]");
    }
}
