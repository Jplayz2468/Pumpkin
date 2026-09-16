// Exact VoxelShape.clip results and interaction-face overrides from Java 26.2.
// Includes real block shape unions, boundaries, inside rays, and translated cells.
import java.util.*;
import net.minecraft.core.*;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.phys.*;
import net.minecraft.world.phys.shapes.*;
public class BlockClipOracle {
    static String bits(double... values) {
        var out=new StringJoiner(",","[","]");
        for(double value:values)out.add(Long.toUnsignedString(Double.doubleToRawLongBits(value)));
        return out.toString();
    }
    static String hit(BlockHitResult hit) {
        if(hit==null)return "null";
        var p=hit.getLocation();
        return "["+hit.getDirection().ordinal()+","+hit.isInside()+","+bits(p.x,p.y,p.z)+"]";
    }
    static String boxes(VoxelShape shape) {
        var out=new StringJoiner(",","[","]");
        for(var b:shape.toAabbs())out.add(bits(b.minX,b.minY,b.minZ,b.maxX,b.maxY,b.maxZ));
        return out.toString();
    }
    public static void main(String[] args) {
        var out=System.out;
        net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        Random random=new Random(262);
        var special=new ArrayList<Integer>();
        for(var s:Block.BLOCK_STATE_REGISTRY)if(!s.getInteractionShape(EmptyBlockGetter.INSTANCE,BlockPos.ZERO).isEmpty())special.add(Block.getId(s));
        out.println("[");
        for(int i=0;i<5000;i++) {
            int id=i%3==0?special.get(i%special.size()):random.nextInt(Block.BLOCK_STATE_REGISTRY.size());
            var state=Block.stateById(id);
            var pos=new BlockPos(i%4==0?10000:0,i%4==0?-64:0,i%4==0?-17:0);
            var shape=state.getShape(EmptyBlockGetter.INSTANCE,pos);
            var from=new Vec3((random.nextInt(49)-16)/16.0,(random.nextInt(49)-16)/16.0,(random.nextInt(49)-16)/16.0);
            var to=new Vec3((random.nextInt(49)-16)/16.0,(random.nextInt(49)-16)/16.0,(random.nextInt(49)-16)/16.0);
            switch(i%13) {
                case 0 -> { from=new Vec3(-1,.5,.5); to=new Vec3(0,.5,.5); }
                case 1 -> { from=new Vec3(.5,.5,.5); to=from.add(1,1,1); }
                case 2 -> { to=from.add(1e-8,0,0); }
                case 3 -> { from=new Vec3(.5,2,.5); to=new Vec3(.5,-1,.5); }
                case 4 -> { from=new Vec3(0,.5,.5); to=new Vec3(1,.5,.5); }
                case 5 -> { from=new Vec3(.5,.5,.5); to=from.add(-1,-1,-1); }
                case 6 -> { from=new Vec3(-1,.5,.5); to=new Vec3(2,.5,.5); }
                case 7 -> { from=new Vec3(.5,.5,-1); to=new Vec3(.5,.5,2); }
            }
            from=from.add(pos.getX(),pos.getY(),pos.getZ()); to=to.add(pos.getX(),pos.getY(),pos.getZ());
            var raw=shape.clip(from,to,pos);
            var result=EmptyBlockGetter.INSTANCE.clipWithInteractionOverride(from,to,pos,shape,state);
            out.println((i==0?"":",")+"["+id+",["+pos.getX()+","+pos.getY()+","+pos.getZ()+"],"+bits(from.x,from.y,from.z)+","+bits(to.x,to.y,to.z)+","+boxes(shape)+","+hit(raw)+","+hit(result)+"]");
        }
        out.println("]");
    }
}
