// Export exact coordinate grids; flattened collision AABBs do not preserve them.
import java.util.*;
import net.minecraft.core.*;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
public class CollisionStepCoordinates {
    public static void main(String[] args) {
        var out=System.out;
        net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        out.println("[");
        int count=0;
        for(BlockState state:Block.BLOCK_STATE_REGISTRY) {
            var shape=state.getCollisionShape(EmptyBlockGetter.INSTANCE,BlockPos.ZERO);
            StringJoiner axes=new StringJoiner(",","[","]");
            for(Direction.Axis axis:Direction.Axis.values()) {
                StringJoiner coords=new StringJoiner(",","[","]");
                if(!shape.isEmpty()) for(double value:shape.getCoords(axis))coords.add(Double.toString(value));
                axes.add(coords.toString());
            }
            out.println((count++==0?"":",")+"["+Block.getId(state)+","+axes+"]");
        }
        out.println("]");
    }
}
