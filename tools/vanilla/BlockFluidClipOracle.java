// BlockGetter clips in small real-state scenes, exercising all four fluid modes.
import java.util.*;
import net.minecraft.core.*;
import net.minecraft.world.level.*;
import net.minecraft.world.level.block.*;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.phys.*;
import net.minecraft.world.phys.shapes.CollisionContext;
public class BlockFluidClipOracle {
    record Scene(BlockState base,BlockState above) implements BlockGetter {
        public BlockState getBlockState(BlockPos pos) { return pos.equals(BlockPos.ZERO)?base:pos.equals(BlockPos.ZERO.above())?above:Blocks.AIR.defaultBlockState(); }
        public FluidState getFluidState(BlockPos pos) { return getBlockState(pos).getFluidState(); }
        public BlockEntity getBlockEntity(BlockPos pos) { return null; }
        public int getHeight() { return 384; }
        public int getMinY() { return -64; }
    }
    public static void main(String[] args) {
        var out=System.out;
        net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var states=new ArrayList<BlockState>();
        for(var state:Block.BLOCK_STATE_REGISTRY) {
            if(!state.getFluidState().isEmpty() || state.is(Blocks.STONE) || state.is(Blocks.OAK_SLAB) || state.is(Blocks.OAK_FENCE))states.add(state);
        }
        // WATER mode tests need the actual built-in water tag; bind it explicitly after Bootstrap.
        var waterTag=net.minecraft.tags.FluidTags.WATER;
        for(var fluid:List.of(net.minecraft.world.level.material.Fluids.WATER,net.minecraft.world.level.material.Fluids.FLOWING_WATER)) {
            var holder=net.minecraft.core.registries.BuiltInRegistries.FLUID.wrapAsHolder(fluid);
            try {
                var method=holder.getClass().getDeclaredMethod("bindTags",java.util.Collection.class); method.setAccessible(true); method.invoke(holder,List.of(waterTag));
            } catch(Exception e) { throw new RuntimeException(e); }
        }
        Random random=new Random(262);
        out.println("[");
        for(int i=0;i<1000;i++) {
            var state=states.get(random.nextInt(states.size()));
            var above=i%3==0?Blocks.WATER.defaultBlockState():Blocks.AIR.defaultBlockState();
            var scene=new Scene(state,above);
            var from=new Vec3(-1,(random.nextInt(15)+1)/16.0,(random.nextInt(15)+1)/16.0);
            var to=new Vec3(2,(random.nextInt(15)+1)/16.0,(random.nextInt(15)+1)/16.0);
            if(i%7==0) { from=new Vec3(.5,.9,.5); to=new Vec3(.5,-.5,.5); }
            if(i%11==0) { from=new Vec3(-1,.95,.5); to=new Vec3(2,.95,.5); }
            var mode=ClipContext.Fluid.values()[i%4];
            var context=new ClipContext(from,to,ClipContext.Block.OUTLINE,mode,CollisionContext.empty());
            var hit=scene.clip(context);
            // Heights are recorded from FluidState.getHeight; EMPTY is kept zero.
            double height=state.getFluidState().isEmpty()?0:state.getFluidState().getHeight(scene,BlockPos.ZERO);
            out.println((i==0?"":",")+"["+Block.getId(state)+","+Block.getId(above)+","+(i%4)+","+BlockClipOracle.bits(from.x,from.y,from.z)+","+BlockClipOracle.bits(to.x,to.y,to.z)+","+BlockClipOracle.bits(height)+","+(hit.getType()==HitResult.Type.MISS?"null":BlockClipOracle.hit(hit))+"]");
        }
        out.println("]");
    }
}
