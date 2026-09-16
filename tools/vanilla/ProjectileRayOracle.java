// Calls Java 26.2 ProjectileUtil's age margin and nearest-entity AABB query.
// Controlled candidates isolate clipping, strict ties, starts inside and endpoints.
import java.util.*;
import java.util.function.Predicate;
import net.minecraft.world.entity.*;
import net.minecraft.world.entity.projectile.ProjectileUtil;
import net.minecraft.world.phys.*;
import sun.misc.Unsafe;
public class ProjectileRayOracle {
    static class Scene extends FluidInteractionOracle.Scene {
        List<Entity> nearby;
        public List<Entity> getEntities(Entity except,AABB bounds,Predicate<? super Entity> predicate) { return nearby; }
    }
    public static void main(String[] args) throws Exception {
        var out=System.out;net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
        var field=Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);var unsafe=(Unsafe)field.get(null);
        var source=(FallDistanceOracle.Probe)unsafe.allocateInstance(FallDistanceOracle.Probe.class);
        var scene=(Scene)unsafe.allocateInstance(Scene.class);
        Random random=new Random(262);out.println("[");
        for(int i=0;i<1200;i++) {
            source.tickCount=i%20;
            if(i%101==0)source.tickCount=Integer.MIN_VALUE;
            if(i%103==0)source.tickCount=Integer.MAX_VALUE;
            Vec3 from=new Vec3(random.nextDouble()*6-3,random.nextDouble()*6-3,random.nextDouble()*6-3);
            Vec3 to=new Vec3(random.nextDouble()*6-3,random.nextDouble()*6-3,random.nextDouble()*6-3);
            switch(i%8) {
                case 0 -> {from=new Vec3(.5,.5,.5);to=new Vec3(3,.5,.5);}
                case 1 -> {from=new Vec3(-1,.5,.5);to=new Vec3(0,.5,.5);}
                case 2 -> {from=new Vec3(0,.5,.5);to=new Vec3(1,.5,.5);}
                case 3 -> {from=new Vec3(-1,-1,-1);to=new Vec3(2,2,2);}
                case 4 -> {from=new Vec3(-5e-8,.5,.5);to=from.add(1e-7,0,0);}
                case 5 -> {from=new Vec3(-1,1+5e-8,.5);to=new Vec3(2,1+5e-8,.5);}
            }
            var boxes=new StringJoiner(",","[","]");scene.nearby=new ArrayList<>();
            for(int j=0;j<5;j++) {
                AABB box=j<2?new AABB(0,0,0,1,1,1):new AABB(random.nextDouble()*4-2,random.nextDouble()*4-2,random.nextDouble()*4-2,random.nextDouble()*4-2,random.nextDouble()*4-2,random.nextDouble()*4-2);
                var candidate=(FallDistanceOracle.Probe)unsafe.allocateInstance(FallDistanceOracle.Probe.class);candidate.setBoundingBox(box);scene.nearby.add(candidate);boxes.add(FluidInteractionOracle.box(box));
            }
            float margin=ProjectileUtil.computeMargin(source);
            var hit=ProjectileUtil.getEntityHitResult(scene,source,from,to,new AABB(from,to),e->true,margin);
            String result=hit==null?"null":"["+java.util.stream.IntStream.range(0,scene.nearby.size()).filter(n->scene.nearby.get(n)==hit.getEntity()).findFirst().orElseThrow()+","+BlockClipOracle.bits(hit.getLocation().x,hit.getLocation().y,hit.getLocation().z)+"]";
            out.println((i==0?"":",")+"["+source.tickCount+","+Float.floatToRawIntBits(margin)+","+BlockClipOracle.bits(from.x,from.y,from.z)+","+BlockClipOracle.bits(to.x,to.y,to.z)+","+boxes+","+result+"]");
        }
        out.println("]");
    }
}
