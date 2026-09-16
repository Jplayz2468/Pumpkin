// Invokes Entity's real Java 26.2 post-collision restitution with a non-living probe.
import java.util.*;
import java.lang.reflect.*;
import net.minecraft.world.entity.*;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.world.level.storage.*;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.*;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.gameevent.GameEvent;
import net.minecraft.core.Holder;
import net.minecraft.world.phys.Vec3;
import sun.misc.Unsafe;
public class CollisionRestitutionOracle {
    static class Probe extends Entity {
        Vec3 velocity; double bounce, gravity; float drag; boolean suppress, bounced;
        Probe() { super(null,null); }
        public Vec3 getDeltaMovement() { return velocity; }
        public void setDeltaMovement(Vec3 value) { velocity=value; }
        protected double getEntityBounciness() { return bounce; }
        protected double getEffectiveGravity() { return gravity; }
        protected float getAirDrag() { return drag; }
        public boolean isSuppressingBounce() { return suppress; }
        public void gameEvent(Holder<GameEvent> event) { bounced=true; }
        public boolean hurtServer(ServerLevel level, DamageSource source, float damage) { return false; }
        protected void defineSynchedData(SynchedEntityData.Builder builder) {}
        protected void readAdditionalSaveData(ValueInput input) {}
        protected void addAdditionalSaveData(ValueOutput output) {}
    }
    static String bits(double... values) {
        var out=new StringJoiner(",","[","]");
        for(double value:values) out.add(Long.toUnsignedString(Double.doubleToRawLongBits(value)));
        return out.toString();
    }
    public static void main(String[] args) throws Exception {
        var out=System.out; net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        // Bootstrap does not load datapack tags. Vanilla 26.2's
        // data/minecraft/tags/block/suppresses_bounce.json contains honey_block.
        var tags=Holder.Reference.class.getDeclaredMethod("bindTags",Collection.class); tags.setAccessible(true);
        tags.invoke(Blocks.HONEY_BLOCK.builtInRegistryHolder(),List.of(net.minecraft.tags.BlockTags.SUPPRESSES_BOUNCE));
        var field=Unsafe.class.getDeclaredField("theUnsafe"); field.setAccessible(true);
        var probe=(Probe)((Unsafe)field.get(null)).allocateInstance(Probe.class);
        var method=Entity.class.getDeclaredMethod("restituteMovementAfterCollisions",BlockState.class,boolean.class,boolean.class,Vec3.class); method.setAccessible(true);
        Random random=new Random(262);
        Block[] blocks={Blocks.STONE,Blocks.SLIME_BLOCK,Blocks.BED.red(),Blocks.HONEY_BLOCK};
        out.println("[");
        for(int i=0;i<800;i++) {
            var block=blocks[i%blocks.length];
            var velocity=new Vec3((random.nextInt(33)-16)/8.0,(random.nextInt(33)-16)/8.0,(random.nextInt(33)-16)/8.0);
            var actual=velocity.scale(random.nextInt(9)/8.0);
            boolean x=(i&1)!=0,z=(i&2)!=0,vertical=(i&4)!=0,below=(i&8)!=0;
            probe.bounce=new double[]{0,.25,.75,1}[i%4]; probe.gravity=new double[]{0,.04,.08,.01}[i/4%4]; probe.drag=new float[]{.98f,.91f,1f}[i%3]; probe.suppress=i%7==0;
            probe.velocity=velocity; probe.verticalCollision=vertical; probe.verticalCollisionBelow=below; probe.bounced=false;
            method.invoke(probe,block.defaultBlockState(),x,z,actual);
            var result=probe.velocity;
            out.println((i==0?"":",")+"["+Block.getId(block.defaultBlockState())+","+bits(velocity.x,velocity.y,velocity.z)+","+bits(actual.x,actual.y,actual.z)+","+x+","+z+","+vertical+","+below+","+probe.suppress+","+bits(probe.bounce,probe.gravity,probe.drag)+","+bits(result.x,result.y,result.z)+","+probe.bounced+"]");
        }
        out.println("]");
    }
}
