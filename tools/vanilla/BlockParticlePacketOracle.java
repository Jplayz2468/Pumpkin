// Serialize real Java 26.2 block-particle packets through the registry stream codec.
import java.util.*;
import io.netty.buffer.*;
import net.minecraft.core.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.particles.*;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.protocol.game.ClientboundLevelParticlesPacket;
import net.minecraft.world.level.block.Block;
public class BlockParticlePacketOracle {
    public static void main(String[] args) {
        var out=System.out; net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var registries=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
        var random=new Random(262); out.println("[");
        for(int i=0;i<128;i++) {
            int state=random.nextInt(Block.BLOCK_STATE_REGISTRY.size());
            boolean overrideLimiter=i%2==0,alwaysShow=i%3==0;
            double x=(i-64)*.125,y=i*.25,z=-i*.5;
            var packet=new ClientboundLevelParticlesPacket(new BlockParticleOption(ParticleTypes.BLOCK,Block.stateById(state)),
                overrideLimiter,alwaysShow,x,y,z,.1F,.2F,.3F,.15F,i*3);
            var buffer=new RegistryFriendlyByteBuf(Unpooled.buffer(),registries);
            ClientboundLevelParticlesPacket.STREAM_CODEC.encode(buffer,packet);
            out.println((i==0?"":",")+"["+state+",\""+ByteBufUtil.hexDump(buffer)+"\"]");
            buffer.release();
        }
        out.println("]");
    }
}
