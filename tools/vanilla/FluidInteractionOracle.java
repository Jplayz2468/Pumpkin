// Runs the actual Java 26.2 EntityFluidInteraction against controlled block/chunk fixtures.
// Only world storage is stubbed: scan order, fluid tracking, current accumulation,
// first-class fluid states, and boat passenger-box clipping are the vanilla methods.
import java.util.*;
import net.minecraft.core.*;
import net.minecraft.tags.FluidTags;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.*;
import net.minecraft.world.level.block.*;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.*;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.material.*;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.*;
import net.minecraft.world.entity.vehicle.boat.*;
import net.minecraft.world.phys.*;
import sun.misc.Unsafe;
public class FluidInteractionOracle {
    static Unsafe unsafe;
    static void set(Class<?> type,Object object,String name,Object value) throws Exception {
        var f=type.getDeclaredField(name); f.setAccessible(true); f.set(object,value);
    }
    static class Chunk extends LevelChunk {
        LevelChunkSection[] sections;
        Chunk() { super(null,new ChunkPos(0,0)); }
        public int getMinY() { return -16; }
        public int getHeight() { return 48; }
        public LevelChunkSection[] getSections() { return sections; }
    }
    static class Scene extends ServerLevel {
        Map<BlockPos,BlockState> states;
        Chunk chunk;
        boolean missing;
        StringJoiner visited;
        Scene() { super(null,null,null,null,null,null,false,0,List.of(),false); }
        public BlockState getBlockState(BlockPos pos) { return states.getOrDefault(pos,Blocks.AIR.defaultBlockState()); }
        public FluidState getFluidState(BlockPos pos) { return getBlockState(pos).getFluidState(); }
        public ChunkAccess getChunk(int x,int z,ChunkStatus status,boolean generate) {
            visited.add("["+x+","+z+"]"); return missing&&x==-1?null:chunk;
        }
    }
    static String box(AABB box) { return box==null?"null":BlockClipOracle.bits(box.minX,box.minY,box.minZ,box.maxX,box.maxY,box.maxZ); }
    public static void main(String[] args) throws Exception {
        var out=System.out; net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        var f=Unsafe.class.getDeclaredField("theUnsafe"); f.setAccessible(true); unsafe=(Unsafe)f.get(null);
        for(var fluid:List.of(Fluids.WATER,Fluids.FLOWING_WATER,Fluids.LAVA,Fluids.FLOWING_LAVA)) {
            var holder=BuiltInRegistries.FLUID.wrapAsHolder(fluid);
            var bind=holder.getClass().getDeclaredMethod("bindTags",Collection.class); bind.setAccessible(true);
            bind.invoke(holder,List.of(fluid.isSame(Fluids.WATER)?FluidTags.WATER:FluidTags.LAVA));
        }
        var level=(Scene)unsafe.allocateInstance(Scene.class);
        level.chunk=(Chunk)unsafe.allocateInstance(Chunk.class);
        var section=(LevelChunkSection)unsafe.allocateInstance(LevelChunkSection.class);
        set(LevelChunkSection.class,section,"fluidCount",(short)1);
        level.chunk.sections=new LevelChunkSection[]{section,section,section};
        var entity=(FallDistanceOracle.Probe)unsafe.allocateInstance(FallDistanceOracle.Probe.class);
        set(Entity.class,entity,"level",level);
        var boat=(Boat)unsafe.allocateInstance(Boat.class);
        var trackerMap=EntityFluidInteraction.class.getDeclaredField("trackerByFluid"); trackerMap.setAccessible(true);
        Random random=new Random(262); out.println("[");
        for(int i=0;i<500;i++) {
            double x=(random.nextInt(17)-8)/4.0,y=(random.nextInt(9)-4)/4.0,z=(random.nextInt(17)-8)/4.0;
            double width=new double[]{.6,1.4,2.0}[i%3],height=new double[]{.5,1.8,2.9}[i%3];
            if(i%17==0) { width=0; height=.001; }
            float eye=new float[]{.4F,1.62F,2.3F}[i%3];
            var original=new AABB(x-width/2,y,z-width/2,x+width/2,y+height,z+width/2);
            entity.setBoundingBox(original); set(Entity.class,entity,"position",new Vec3(x,y,z));
            set(Entity.class,entity,"blockPosition",BlockPos.containing(x,y,z)); set(Entity.class,entity,"eyeHeight",eye);
            var boatBox=new AABB(x-1,y-.25,z-1,x+1,y+new double[]{.1,.6,3}[i%3],z+1);
            boat.setBoundingBox(boatBox);
            var status=AbstractBoat.Status.values()[i%5]; set(AbstractBoat.class,boat,"status",status);
            boolean riding=i%4==0; set(Entity.class,entity,"vehicle",riding?boat:null);
            level.states=new HashMap<>(); level.visited=new StringJoiner(",","[","]"); level.missing=i%7==0;
            for(int xx=-4;xx<=4;xx++)for(int yy=-2;yy<=4;yy++)for(int zz=-4;zz<=4;zz++) {
                int r=random.nextInt(14);
                var state=r<2?Blocks.WATER.defaultBlockState():r==2?Blocks.LAVA.defaultBlockState():Blocks.AIR.defaultBlockState();
                if(r==3)state=Blocks.WATER.defaultBlockState().setValue(net.minecraft.world.level.block.state.properties.BlockStateProperties.LEVEL,4);
                if(!state.isAir())level.states.put(new BlockPos(xx,yy,zz),state);
            }
            var cells=new StringJoiner(",","[","]");
            for(var entry:level.states.entrySet()) {
                var pos=entry.getKey(); var fluid=entry.getValue().getFluidState(); var flow=fluid.getFlow(level,pos);
                cells.add("[["+pos.getX()+","+pos.getY()+","+pos.getZ()+"],"+(fluid.is(FluidTags.WATER)?0:1)+","+BlockClipOracle.bits(fluid.getHeight(level,pos))+","+BlockClipOracle.bits(flow.x,flow.y,flow.z)+"]");
            }
            boolean ignore=i%5==0;
            var interaction=new EntityFluidInteraction(Set.of(FluidTags.WATER,FluidTags.LAVA)); interaction.update(entity,ignore);
            var results=new StringJoiner(",","[","]");
            var trackers=(Map<?,?>)trackerMap.get(interaction);
            for(var tag:List.of(FluidTags.WATER,FluidTags.LAVA)) {
                var tracker=trackers.get(tag); var t=tracker.getClass();
                var current=t.getDeclaredField("accumulatedCurrent");current.setAccessible(true);
                var count=t.getDeclaredField("currentCount");count.setAccessible(true);
                var flow=(Vec3)current.get(tracker);
                results.add("["+BlockClipOracle.bits(interaction.getFluidHeight(tag))+","+interaction.isEyeInFluid(tag)+","+BlockClipOracle.bits(flow.x,flow.y,flow.z)+","+count.getInt(tracker)+"]");
            }
            out.println((i==0?"":",")+"["+box(original)+","+BlockClipOracle.bits(x,y+eye,z)+","+riding+","+boat.isUnderWater()+","+box(boatBox)+","+box(entity.getFluidInteractionBox())+","+ignore+","+level.missing+","+cells+","+level.visited+","+results+"]");
        }
        out.println("]");
    }
}
