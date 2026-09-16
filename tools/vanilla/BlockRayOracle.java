// Real Java 26.2 BlockGetter traversal and VoxelShape.clip, including boundary rays.
import java.util.*;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.core.BlockPos;
import net.minecraft.world.phys.*;
import net.minecraft.world.phys.shapes.Shapes;
public class BlockRayOracle {
    static String bits(double... values) {
        var out=new StringJoiner(",","[","]");
        for(double value:values)out.add(Long.toUnsignedString(Double.doubleToRawLongBits(value)));
        return out.toString();
    }
    public static void main(String[] args) {
        Random random=new Random(262);
        System.out.println("[");
        for(int i=0;i<600;i++) {
            Vec3 from=new Vec3((random.nextInt(25)-12)/4.0,(random.nextInt(25)-12)/4.0,(random.nextInt(25)-12)/4.0);
            Vec3 to=new Vec3((random.nextInt(25)-12)/4.0,(random.nextInt(25)-12)/4.0,(random.nextInt(25)-12)/4.0);
            if(i%5==0) to=from.add(i%7==0?1e-8:2,0,0);
            if(i%11==0) to=from;
            double height=new double[]{1,.125,.8888888955116272}[i%3];
            var shape=Shapes.box(0,0,0,1,height,1);
            var positions=new StringJoiner(",","[","]");
            BlockGetter.traverseBlocks(from,to,null,(context,pos)->{ positions.add("["+pos.getX()+","+pos.getY()+","+pos.getZ()+"]"); return null; },context->null);
            System.out.println((i==0?"":",")+"["+bits(from.x,from.y,from.z)+","+bits(to.x,to.y,to.z)+","+bits(height)+","+positions+","+(shape.clip(from,to,BlockPos.ZERO)!=null)+"]");
        }
        System.out.println("]");
    }
}
