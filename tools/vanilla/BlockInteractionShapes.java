// Export every nonempty interaction shape from the unmodified Java 26.2 server.
import java.util.*;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
public class BlockInteractionShapes {
    public static void main(String[] args) {
        var out=System.out;
        net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var entries=new StringJoiner(",\n","[\n","\n]");
        for(var state:Block.BLOCK_STATE_REGISTRY) {
            var shape=state.getInteractionShape(EmptyBlockGetter.INSTANCE,BlockPos.ZERO);
            if(shape.isEmpty())continue;
            var boxes=new StringJoiner(",","[","]");
            for(var box:shape.toAabbs()) boxes.add("["+box.minX+","+box.minY+","+box.minZ+","+box.maxX+","+box.maxY+","+box.maxZ+"]");
            entries.add("["+Block.getId(state)+","+boxes+"]");
        }
        out.println(entries);
    }
}
