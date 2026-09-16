// Actual Projectile owner-exit check and canHitEntity, with controlled vehicle trees.
import java.util.*;
import java.util.stream.Stream;
import net.minecraft.world.entity.*;
import net.minecraft.world.entity.projectile.Projectile;
import net.minecraft.world.phys.*;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.damagesource.DamageSource;
import sun.misc.Unsafe;
public class ProjectileOwnerOracle {
    static class Target extends FallDistanceOracle.Probe {
        Target root;
        List<Entity> tree;
        boolean pickable,canHit;
        int queries;
        public Entity getRootVehicle() { return root==null?this:root; }
        public Stream<Entity> getSelfAndPassengers() { queries++;return tree.stream(); }
        public boolean isPickable() { return pickable; }
        public boolean canBeHitByProjectile() { return canHit; }
    }
    static class Probe extends Projectile {
        Entity ownerTarget;
        Probe() { super(null,null); }
        public Entity getOwner() { return ownerTarget; }
        protected void defineSynchedData(SynchedEntityData.Builder builder) {}
        public boolean hurtServer(ServerLevel level,DamageSource source,float damage) { return false; }
        void check() { checkLeftOwner(); }
        boolean canHit(Entity target) { return canHitEntity(target); }
    }
    public static void main(String[] args) throws Exception {
        var out=System.out;net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
        var field=Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);var unsafe=(Unsafe)field.get(null);
        var probe=(Probe)unsafe.allocateInstance(Probe.class);
        var root=(Target)unsafe.allocateInstance(Target.class);
        var owner=(Target)unsafe.allocateInstance(Target.class);owner.root=root;
        var same=(Target)unsafe.allocateInstance(Target.class);same.root=root;
        var other=(Target)unsafe.allocateInstance(Target.class);
        var left=Projectile.class.getDeclaredField("leftOwner");left.setAccessible(true);
        var checked=Projectile.class.getDeclaredField("leftOwnerChecked");checked.setAccessible(true);
        Random random=new Random(262);out.println("[");
        for(int i=0;i<1200;i++) {
            boolean reset=i%12==0,begin=i%2==0;
            if(reset){left.setBoolean(probe,false);checked.setBoolean(probe,false);}
            if(begin)checked.setBoolean(probe,false);
            probe.ownerTarget=i%53==0?null:owner;
            double x=i%12<6?0:4;
            probe.setBoundingBox(new AABB(x,.4,0,x+.25,.65,.25));
            probe.setDeltaMovement(new Vec3((random.nextDouble()-.5)*2,0,(random.nextDouble()-.5)*2));
            var search=probe.getBoundingBox().expandTowards(probe.getDeltaMovement()).inflate(1);
            root.tree=new ArrayList<>();var members=new StringJoiner(",","[","]");
            for(int j=0;j<4;j++) {
                var member=j==0?root:j==1?owner:(Target)unsafe.allocateInstance(Target.class);
                member.pickable=i%17!=0&&j%3!=2;
                double offset=j==0?0:j==1?.5:random.nextDouble()*4;
                member.setBoundingBox(new AABB(offset,0,0,offset+.6,1.8,.6));root.tree.add(member);
                members.add("["+member.pickable+","+FluidInteractionOracle.box(member.getBoundingBox())+"]");
            }
            root.queries=0;probe.check();
            same.canHit=other.canHit=i%3!=0;
            out.println((i==0?"":",")+"["+reset+","+begin+","+(probe.ownerTarget!=null)+","+FluidInteractionOracle.box(search)+","+members+","+left.getBoolean(probe)+","+root.queries+","+same.canHit+","+probe.canHit(same)+","+probe.canHit(other)+"]");
        }
        out.println("]");
    }
}
