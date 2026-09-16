// Uses the real Java 26.2 shape clipper and step candidate collector.
import java.util.*;
import java.lang.reflect.Method;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.phys.*;
import net.minecraft.world.phys.shapes.*;
public class StepCollisionOracle {
    static String bits(double... xs) {
        StringJoiner result = new StringJoiner(",", "[", "]");
        for (double x : xs) result.add(Long.toUnsignedString(Double.doubleToRawLongBits(x)));
        return result.toString();
    }
    static String box(AABB a) { return "[" + bits(a.minX,a.minY,a.minZ) + "," + bits(a.maxX,a.maxY,a.maxZ) + "]"; }
    static Vec3 clip(Method method, Vec3 motion, AABB box, List<VoxelShape> shapes) throws Exception {
        return (Vec3)method.invoke(null, motion, box, shapes);
    }
    public static void main(String[] args) throws Exception {
        var out = System.out;
        net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        Method clip = Entity.class.getDeclaredMethod("collideWithShapes",Vec3.class,AABB.class,List.class); clip.setAccessible(true);
        Method heights = Entity.class.getDeclaredMethod("collectCandidateStepUpHeights", AABB.class,List.class,float.class,float.class); heights.setAccessible(true);
        Random random = new Random(262);
        out.println("[");
        for (int i=0; i<400; i++) {
            double y = (i%5==0 ? 0.25 : 0.0);
            AABB bounds = new AABB(-0.3,y,-0.3,0.3,y+1.8,0.3);
            Vec3 motion = new Vec3((random.nextInt(33)-16)/8.0,-random.nextInt(9)/8.0,(random.nextInt(33)-16)/8.0);
            float max = new float[]{0f,0.5f,0.6f,1f,1.5f}[i%5];
            boolean ground = i%3!=0;
            List<VoxelShape> shapes = new ArrayList<>();
            List<AABB> boxes = new ArrayList<>();
            boxes.add(new AABB(-4,-1,-4,4,0,4));
            for (int n=0; n<5; n++) {
                double x=(random.nextInt(25)-12)/8.0, z=(random.nextInt(25)-12)/8.0;
                double h=(random.nextInt(17)+1)/16.0;
                boxes.add(new AABB(x,0,z,x+0.5,h,z+0.5));
            }
            if(i%4==0) boxes.add(new AABB(-3,2,-3,3,2.5,3));
            for(AABB b:boxes) shapes.add(Shapes.create(b));
            Vec3 initial = motion.lengthSqr()==0 ? motion : clip(clip,motion,bounds,shapes);
            Vec3 result = initial;
            boolean landed = motion.y!=initial.y && motion.y<0;
            float[] candidates = new float[0];
            if(max>0 && (landed || ground) && (motion.x!=initial.x || motion.z!=initial.z)) {
                AABB feet = landed ? bounds.move(0,initial.y,0) : bounds;
                candidates=(float[])heights.invoke(null,feet,shapes,max,(float)initial.y);
                for(float height:candidates) {
                    Vec3 step=clip(clip,new Vec3(motion.x,height,motion.z),feet,shapes);
                    if(step.horizontalDistanceSqr()>initial.horizontalDistanceSqr()) {
                        result=step.subtract(0,bounds.minY-feet.minY,0); break;
                    }
                }
            }
            StringJoiner shapeJson=new StringJoiner(",","[","]"); for(AABB b:boxes)shapeJson.add(box(b));
            StringJoiner candidateJson=new StringJoiner(",","[","]"); for(float h:candidates)candidateJson.add(Integer.toUnsignedString(Float.floatToRawIntBits(h)));
            out.println((i==0?"":",")+"{\"bounds\":"+box(bounds)+",\"motion\":"+bits(motion.x,motion.y,motion.z)+",\"grounded\":"+ground+",\"max\":"+max+",\"shapes\":"+shapeJson+",\"initial\":"+bits(initial.x,initial.y,initial.z)+",\"candidates\":"+candidateJson+",\"result\":"+bits(result.x,result.y,result.z)+"}");
        }
        out.println("]");
    }
}
