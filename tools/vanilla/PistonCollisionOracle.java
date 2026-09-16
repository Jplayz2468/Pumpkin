// Java 26.2 dynamic piston geometry and complete optimized coordinate grids.
import java.util.*;
import net.minecraft.core.*;
import net.minecraft.world.level.*;
import net.minecraft.world.level.block.*;
import net.minecraft.world.level.block.piston.*;
import net.minecraft.world.phys.shapes.*;
public class PistonCollisionOracle {
    static long mix(long hash, double value) { return (hash ^ Double.doubleToRawLongBits(value == 0 ? 0 : value)) * 0x100000001b3L; }
    public static void main(String[] args) throws Exception {
        var out=System.out;
        net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var progress=PistonMovingBlockEntity.class.getDeclaredField("progress"); progress.setAccessible(true);
        var noClip=PistonMovingBlockEntity.class.getDeclaredField("NOCLIP"); noClip.setAccessible(true);
        @SuppressWarnings("unchecked") var local=(ThreadLocal<Direction>)noClip.get(null);
        Block[] blocks={Blocks.STONE,Blocks.STONE_SLAB,Blocks.OAK_STAIRS,Blocks.OAK_FENCE,Blocks.PISTON,Blocks.STICKY_PISTON,Blocks.PISTON_HEAD,Blocks.SCAFFOLDING,Blocks.CHEST,Blocks.HOPPER};
        out.println("["); boolean first=true;
        for(Block block:blocks) for(var state:block.getStateDefinition().getPossibleStates())
        for(Direction direction:Direction.values()) for(boolean extending:new boolean[]{false,true})
        for(boolean source:new boolean[]{false,true}) for(float value:new float[]{0,.1f,.25f,.5f,.74999994f,.75f,.75000006f,.99999994f,1})
        for(boolean noclip:new boolean[]{false,true}) {
            var piston=new PistonMovingBlockEntity(BlockPos.ZERO,Blocks.MOVING_PISTON.defaultBlockState(),state,direction,extending,source);
            progress.setFloat(piston,value);
            if(noclip) local.set(extending?direction:direction.getOpposite()); else local.remove();
            var shape=piston.getCollisionShape(EmptyBlockGetter.INSTANCE,BlockPos.ZERO);
            long hash=0xcbf29ce484222325L;
            for(var axis:Direction.Axis.values()) {
                var coords=shape.getCoords(axis);
                hash=mix(hash,coords.size());
                for(double coordinate:coords) hash=mix(hash,coordinate);
            }
            var boxes=shape.toAabbs(); hash=mix(hash,boxes.size());
            for(var box:boxes) for(double coordinate:new double[]{box.minX,box.minY,box.minZ,box.maxX,box.maxY,box.maxZ}) hash=mix(hash,coordinate);
            out.println((first?"":",")+"["+Block.getId(state)+",\""+direction.getName()+"\","+extending+","+source+","+value+","+noclip+","+Long.toUnsignedString(hash)+"]"); first=false;
        }
        local.remove(); out.println("]");
    }
}
