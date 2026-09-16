// Actual FlowingFluid.getFlow in real block-state neighborhoods, no flow mocks.
import java.util.*;
import net.minecraft.core.*;
import net.minecraft.world.level.block.*;
import net.minecraft.world.level.block.state.*;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
public class FluidFlowOracle {
    static class Scene implements net.minecraft.world.level.BlockGetter {
        Map<BlockPos,BlockState> states;
        public BlockState getBlockState(BlockPos pos) { return states.getOrDefault(pos,Blocks.AIR.defaultBlockState()); }
        public net.minecraft.world.level.material.FluidState getFluidState(BlockPos pos) { return getBlockState(pos).getFluidState(); }
        public net.minecraft.world.level.block.entity.BlockEntity getBlockEntity(BlockPos pos) { return null; }
        public int getHeight() { return 384; }
        public int getMinY() { return -64; }
    }
    public static void main(String[] args) throws Exception {
        var out=System.out; net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var scene=new Scene();
        var random=new Random(262); var states=new ArrayList<BlockState>();
        for(var block:List.of(Blocks.WATER,Blocks.LAVA,Blocks.AIR,Blocks.STONE,Blocks.OAK_SLAB,Blocks.OAK_STAIRS,Blocks.ICE,Blocks.FROSTED_ICE,Blocks.PACKED_ICE,Blocks.OAK_FENCE,Blocks.BUBBLE_COLUMN)) states.addAll(block.getStateDefinition().getPossibleStates());
        out.println("[");
        for(int i=0;i<1600;i++) {
            scene.states=new HashMap<>();
            for(int x=-1;x<=1;x++)for(int y=-1;y<=1;y++)for(int z=-1;z<=1;z++)scene.states.put(new BlockPos(x,y,z),states.get(random.nextInt(states.size())));
            var origin=(i%2==0?Blocks.WATER:Blocks.LAVA).defaultBlockState().setValue(BlockStateProperties.LEVEL,i%16);
            scene.states.put(BlockPos.ZERO,origin);
            var flow=origin.getFluidState().getFlow(scene,BlockPos.ZERO);
            var cells=new StringJoiner(",","[","]");
            for(var entry:scene.states.entrySet()) {
                var p=entry.getKey();cells.add("["+p.getX()+","+p.getY()+","+p.getZ()+","+Block.getId(entry.getValue())+"]");
            }
            out.println((i==0?"":",")+"["+cells+","+BlockClipOracle.bits(flow.x,flow.y,flow.z)+"]");
        }
        out.println("]");
    }
}
