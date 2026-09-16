// Actual LivingEntity impulse-context transitions and causeFallDamage distance clamp.
import java.util.*;
import net.minecraft.world.entity.*;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.item.*;
import sun.misc.Unsafe;
public class ImpulseContextOracle {
    static class Probe extends LivingEntity {
        double captured;
        Probe() { super(EntityTypes.ARMOR_STAND,null); }
        public HumanoidArm getMainArm() { return HumanoidArm.RIGHT; }
        protected void propagateFallToPassengers(double d,float m,DamageSource s) {}
        protected int calculateFallDamage(double distance,float multiplier) { captured=distance; return 0; }
    }
    public static void main(String[] args) throws Exception {
        var out=System.out; net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var f=Unsafe.class.getDeclaredField("theUnsafe"); f.setAccessible(true); var unsafe=(Unsafe)f.get(null);
        var probe=(Probe)unsafe.allocateInstance(Probe.class);
        var type=Entity.class.getDeclaredField("type"); type.setAccessible(true); type.set(probe,EntityTypes.ARMOR_STAND);
        var position=Entity.class.getDeclaredField("position"); position.setAccessible(true);
        var grace=LivingEntity.class.getDeclaredField("currentImpulseContextResetGraceTime"); grace.setAccessible(true);
        var mace=MaceItem.class.getDeclaredMethod("calculateImpactPosition",LivingEntity.class); mace.setAccessible(true);
        var random=new Random(262); out.println("[");
        for(int i=0;i<1000;i++) {
            int op=random.nextInt(6); int ticks=random.nextInt(70)-10;
            Vec3 pos=new Vec3(i%5,(random.nextDouble()-.5)*40,i%7);
            position.set(probe,pos); double distance=random.nextDouble()*50;
            String result="null";
            switch(op) {
                case 0 -> probe.setIgnoreFallDamageFromCurrentImpulse(true,pos);
                case 1 -> probe.setIgnoreFallDamageFromCurrentImpulse(false,pos);
                case 2 -> probe.applyPostImpulseGraceTime(ticks);
                case 3 -> probe.tryResetCurrentImpulseContext();
                case 4 -> probe.resetCurrentImpulseContext();
                case 5 -> { probe.causeFallDamage(distance,1,null); result=BlockClipOracle.bits(probe.captured); }
            }
            var impact=probe.currentImpulseImpactPos;
            var macePos=(Vec3)mace.invoke(Items.MACE,probe);
            out.println((i==0?"":",")+"["+op+","+ticks+","+BlockClipOracle.bits(pos.x,pos.y,pos.z)+","+BlockClipOracle.bits(distance)+","+result+","+grace.getInt(probe)+","+(impact==null?"null":BlockClipOracle.bits(impact.x,impact.y,impact.z))+","+BlockClipOracle.bits(macePos.x,macePos.y,macePos.z)+"]");
        }
        out.println("]");
    }
}
