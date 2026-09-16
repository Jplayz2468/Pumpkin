// Fingerprints actual vanilla clipping for every block state, over 81 fixed motions.
import java.util.*;
import java.lang.reflect.Method;
import net.minecraft.core.BlockPos;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.phys.*;
import net.minecraft.world.phys.shapes.*;
public class StaticCollisionOracle {
    public static void main(String[] args) throws Exception {
        var out=System.out;
        net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        Method clip=Entity.class.getDeclaredMethod("collideWithShapes",Vec3.class,AABB.class,List.class); clip.setAccessible(true);
        out.println("["); int index=0;
        for(var state:Block.BLOCK_STATE_REGISTRY) {
            var shape=state.getCollisionShape(EmptyBlockGetter.INSTANCE,BlockPos.ZERO);
            List<VoxelShape> shapes=shape.isEmpty()?List.of():List.of(shape);
            long hash=0xcbf29ce484222325L;
            for(int pose=0;pose<3;pose++) {
                double p=pose*0.5, y=pose*0.25;
                AABB bounds=new AABB(p-0.3,y,p-0.3,p+0.3,y+1.8,p+0.3);
                for(int x=-1;x<=1;x++)for(int v=-1;v<=1;v++)for(int z=-1;z<=1;z++) {
                    Vec3 result=(Vec3)clip.invoke(null,new Vec3(x*1.125,v*0.875,z*1.125),bounds,shapes);
                    for(double value:new double[]{result.x,result.y,result.z}) hash=(hash^(value==0?0:Double.doubleToRawLongBits(value)))*0x100000001b3L;
                }
            }
            out.println((index++==0?"":",")+Long.toUnsignedString(hash));
        }
        out.println("]");
    }
}
