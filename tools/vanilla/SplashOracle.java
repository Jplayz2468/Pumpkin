// Invokes the actual Entity.doWaterSplashEffect; only output sinks are overridden.
import java.util.*;
import net.minecraft.core.Holder;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.util.RandomSource;
import net.minecraft.world.entity.*;
import net.minecraft.world.level.gameevent.GameEvent;
import net.minecraft.world.phys.Vec3;
import sun.misc.Unsafe;
public class SplashOracle {
    static class Probe extends FallDistanceOracle.Probe {
        LivingEntity controller;
        float volume,pitch;
        int events;
        public LivingEntity getControllingPassenger() { return controller; }
        public void playSound(SoundEvent sound,float volume,float pitch) { this.volume=volume; this.pitch=pitch; }
        public void gameEvent(Holder<GameEvent> event) { if(event==GameEvent.SPLASH)events++; }
        void splash() { doWaterSplashEffect(); }
    }
    public static void main(String[] args) throws Exception {
        var out=System.out; net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var f=Unsafe.class.getDeclaredField("theUnsafe"); f.setAccessible(true); var unsafe=(Unsafe)f.get(null);
        var entity=(Probe)unsafe.allocateInstance(Probe.class);
        var rider=(net.minecraft.world.entity.decoration.ArmorStand)unsafe.allocateInstance(net.minecraft.world.entity.decoration.ArmorStand.class);
        var level=(FluidInteractionOracle.Scene)unsafe.allocateInstance(FluidInteractionOracle.Scene.class);
        FluidInteractionOracle.set(Entity.class,entity,"level",level);
        FluidInteractionOracle.set(Entity.class,entity,"position",Vec3.ZERO);
        var random=new Random(262); out.println("[");
        for(int i=0;i<300;i++) {
            long seed=random.nextInt(Integer.MAX_VALUE);
            var rng=RandomSource.create(seed);
            var movement=new Vec3((random.nextDouble()-.5)*10,(random.nextDouble()-.5)*5,(random.nextDouble()-.5)*10);
            float width=new float[]{0,.01F,.3F,.6F,1.4F,2,16}[i%7];
            FluidInteractionOracle.set(Entity.class,entity,"random",rng);
            FluidInteractionOracle.set(Entity.class,entity,"dimensions",EntityDimensions.scalable(width,1.8F));
            entity.controller=i%3==0?rider:null; entity.events=0;
            entity.setDeltaMovement(movement); rider.setDeltaMovement(movement);
            entity.splash();
            out.println((i==0?"":",")+"["+seed+","+BlockClipOracle.bits(movement.x,movement.y,movement.z)+","+(entity.controller!=null)+","+Integer.toUnsignedLong(Float.floatToRawIntBits(width))+","+Integer.toUnsignedLong(Float.floatToRawIntBits(entity.volume))+","+Integer.toUnsignedLong(Float.floatToRawIntBits(entity.pitch))+","+rng.nextLong()+","+entity.events+"]");
        }
        out.println("]");
    }
}
