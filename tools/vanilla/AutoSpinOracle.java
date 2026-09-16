// Actual LivingEntity.checkAutoSpinAttack with captured world query and attack sink.
import java.util.*;
import java.util.function.Predicate;
import net.minecraft.world.entity.*;
import net.minecraft.world.item.*;
import net.minecraft.world.phys.*;
import sun.misc.Unsafe;
public class AutoSpinOracle {
    static class Scene extends FluidInteractionOracle.Scene {
        List<Entity> nearby;
        AABB query;
        public List<Entity> getEntities(Entity except,AABB bounds,Predicate<? super Entity> predicate) { query=bounds;return nearby; }
    }
    static class Probe extends ImpulseContextOracle.Probe {
        boolean flag;
        int attacks;
        protected void setLivingEntityFlag(int flag,boolean value) { if(flag==4)this.flag=value; }
        protected void doAutoAttackOnTouch(LivingEntity target) { attacks++; }
        void init(int ticks) { autoSpinAttackTicks=ticks;autoSpinAttackDmg=8;autoSpinAttackItemStack=ItemStack.EMPTY;flag=true;attacks=0; }
        void run(AABB old,AABB current) { autoSpinAttackTicks--;checkAutoSpinAttack(old,current); }
        String result() { var v=getDeltaMovement();return "["+autoSpinAttackTicks+","+autoSpinAttackDmg+","+(autoSpinAttackItemStack!=null)+","+flag+","+attacks+","+BlockClipOracle.bits(v.x,v.y,v.z)+"]"; }
    }
    public static void main(String[] args) throws Exception {
        var out=System.out; net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var field=Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);var unsafe=(Unsafe)field.get(null);
        var probe=(Probe)unsafe.allocateInstance(Probe.class);
        var scene=(Scene)unsafe.allocateInstance(Scene.class);
        FluidInteractionOracle.set(Entity.class,probe,"level",scene);
        var nonliving=(FallDistanceOracle.Probe)unsafe.allocateInstance(FallDistanceOracle.Probe.class);
        var living=(Probe)unsafe.allocateInstance(Probe.class);
        Random random=new Random(262);out.println("[");
        for(int i=0;i<500;i++) {
            int ticks=1+random.nextInt(22),kind=i%5;
            scene.nearby=switch(kind) {case 0->List.of();case 1->List.of(nonliving);case 2->List.of(living);case 3->List.of(nonliving,living);default->List.of(living,living);};
            var motion=new Vec3(random.nextDouble()*4-2,random.nextDouble()*4-2,random.nextDouble()*4-2);
            probe.init(ticks);probe.horizontalCollision=i%2==0;probe.setDeltaMovement(motion);
            var before=new AABB(0,0,0,.6,1.8,.6);var after=before.move(motion);
            probe.run(before,after);
            out.println((i==0?"":",")+"["+ticks+","+kind+","+probe.horizontalCollision+","+BlockClipOracle.bits(motion.x,motion.y,motion.z)+","+probe.result()+"]");
        }
        out.println("]");
    }
}
