// Actual CollisionGetter.clipIncludingBorder, with a controlled ordinary block hit.
import java.util.*;
import net.minecraft.core.*;
import net.minecraft.world.level.*;
import net.minecraft.world.level.border.WorldBorder;
import net.minecraft.world.phys.*;
import net.minecraft.world.phys.shapes.CollisionContext;
import sun.misc.Unsafe;
public class BorderRayOracle {
    static class Scene extends FluidInteractionOracle.Scene {
        WorldBorder border;
        BlockHitResult block;
        public WorldBorder getWorldBorder() { return border; }
        public BlockHitResult clip(ClipContext context) { return block; }
    }
    public static void main(String[] args) throws Exception {
        var out=System.out;net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
        var field=Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);var unsafe=(Unsafe)field.get(null);
        var scene=(Scene)unsafe.allocateInstance(Scene.class);scene.border=new WorldBorder();
        Random random=new Random(262);out.println("[");
        for(int i=0;i<500;i++) {
            double centerX=i%3==0?10.25:0,centerZ=i%3==0?-4.75:0;
            double size=i%11==0?1e-6:10;
            scene.border.setCenter(centerX,centerZ);scene.border.setSize(size);
            Vec3 from=new Vec3(centerX+(random.nextDouble()-.5)*size*1.3,2,centerZ+(random.nextDouble()-.5)*size*1.3);
            Vec3 to=new Vec3(centerX+(random.nextDouble()-.5)*size*4,4,centerZ+(random.nextDouble()-.5)*size*4);
            if(i%9==0)from=new Vec3(scene.border.getMinX(),2,centerZ);
            if(i%9==1)from=new Vec3(scene.border.getMaxX(),2,centerZ);
            if(i%9==2)to=new Vec3(scene.border.getMaxX(),4,centerZ);
            if(i%9==3)to=new Vec3(scene.border.getMinX(),4,centerZ);
            if(i%9==4)to=from.add(size*2,size*2,size*2);
            scene.block=BlockHitResult.miss(to,Direction.NORTH,BlockPos.containing(to));
            var context=new ClipContext(from,to,ClipContext.Block.COLLIDER,ClipContext.Fluid.NONE,CollisionContext.empty());
            var hit=scene.clipIncludingBorder(context);
            out.println((i==0?"":",")+"["+BlockClipOracle.bits(scene.border.getMinX(),scene.border.getMinZ(),scene.border.getMaxX(),scene.border.getMaxZ())+","+BlockClipOracle.bits(from.x,from.y,from.z)+","+BlockClipOracle.bits(to.x,to.y,to.z)+","+(hit.isWorldBorderHit()?BlockClipOracle.hit(hit):"null")+"]");
        }
        out.println("]");
    }
}
