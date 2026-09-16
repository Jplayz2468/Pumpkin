// Export canonical SoundEvent ranges and serialize actual positional/entity packets.
import java.util.*;
import io.netty.buffer.*;
import net.minecraft.core.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.protocol.game.*;
import net.minecraft.resources.Identifier;
import net.minecraft.sounds.*;
import net.minecraft.world.entity.Entity;
import sun.misc.Unsafe;
public class SoundPacketOracle {
    public static void main(String[] args) throws Exception {
        var out=System.out;net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
        var ranges=new TreeMap<String,Float>();
        for(var sound:BuiltInRegistries.SOUND_EVENT) sound.fixedRange().ifPresent(range->ranges.put(BuiltInRegistries.SOUND_EVENT.getKey(sound).getPath(),range));
        out.println("{\"registry_size\":"+BuiltInRegistries.SOUND_EVENT.size()+",\"fixed_ranges\":{");boolean first=true;
        for(var entry:ranges.entrySet()) {out.println((first?"":",")+"\""+entry.getKey()+"\":"+entry.getValue());first=false;}
        out.println("},\"packets\":[");
        var f=Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);var unsafe=(Unsafe)f.get(null);
        var entity=(FallDistanceOracle.Probe)unsafe.allocateInstance(FallDistanceOracle.Probe.class);
        var registries=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
        Random random=new Random(262);
        for(int i=0;i<160;i++) {
            int id=random.nextInt(BuiltInRegistries.SOUND_EVENT.size());
            Float range=i%3==0?23.75F:null;
            Holder<SoundEvent> sound=i%2==0?BuiltInRegistries.SOUND_EVENT.wrapAsHolder(BuiltInRegistries.SOUND_EVENT.byId(id)):
                Holder.direct(new SoundEvent(Identifier.parse("minecraft:test.sound"),Optional.ofNullable(range)));
            var category=SoundSource.values()[i%SoundSource.values().length];
            double x=(i-80)*.17,y=i*.31,z=-i*.23;float volume=(i%9)*.35F,pitch=.2F+(i%7)*.33F;
            long seed=random.nextLong();entity.setId(i*19-700);
            var positionPacket=new ClientboundSoundPacket(sound,category,x,y,z,volume,pitch,seed);
            var entityPacket=new ClientboundSoundEntityPacket(sound,category,entity,volume,pitch,seed);
            var positionBuffer=new RegistryFriendlyByteBuf(Unpooled.buffer(),registries);
            var entityBuffer=new RegistryFriendlyByteBuf(Unpooled.buffer(),registries);
            ClientboundSoundPacket.STREAM_CODEC.encode(positionBuffer,positionPacket);
            ClientboundSoundEntityPacket.STREAM_CODEC.encode(entityBuffer,entityPacket);
            out.println((i==0?"":",")+"["+id+","+(i%2==0)+","+range+","+BlockClipOracle.bits(x,y,z)+","+Integer.toUnsignedLong(Float.floatToRawIntBits(volume))+","+Integer.toUnsignedLong(Float.floatToRawIntBits(pitch))+","+seed+",\""+ByteBufUtil.hexDump(positionBuffer)+"\",\""+ByteBufUtil.hexDump(entityBuffer)+"\"]");
            positionBuffer.release();entityBuffer.release();
        }
        out.println("]}");
    }
}
