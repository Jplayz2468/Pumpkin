// Run against the unmodified local Java 26.2 server and its dependency classpath.
// The fixture captures BlockGetter's exact cell order and iteration numbering.
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import java.util.Random;

public class InsideTraversalOracle {
    static String bits(double value) { return Long.toUnsignedString(Double.doubleToRawLongBits(value)); }
    public static void main(String[] args) {
        Random random = new Random(262);
        System.out.println("[");
        for (int i = 0; i < 120; i++) {
            double x = random.nextDouble() * 4 - 2;
            double y = random.nextDouble() * 4 - 2;
            double z = random.nextDouble() * 4 - 2;
            double dx = random.nextDouble() * 12 - 6;
            double dy = random.nextDouble() * 12 - 6;
            double dz = random.nextDouble() * 12 - 6;
            if (i < 27) {
                dx = (i % 3 - 1) * 2.5;
                dy = ((i / 3) % 3 - 1) * 2.5;
                dz = (i / 9 - 1) * 2.5;
            }
            if (i == 27) { x = y = z = 0; dx = dy = dz = 3; }
            if (i == 28) { x = y = z = 0; dx = dy = dz = -3; }
            if (i == 29) { dx = 1.0e-6; dy = dz = 0; }
            if (i == 30) { dx = 1.0e-5F; dy = dz = 0; }
            Vec3 from = new Vec3(x, y, z);
            Vec3 to = from.add(dx, dy, dz);
            double width = i % 4 == 0 ? 2.2 : 0.6;
            AABB target = new AABB(to.x-width/2, to.y, to.z-width/2,
                to.x+width/2, to.y+1.8, to.z+width/2).deflate(1.0e-5F);
            StringBuilder hits = new StringBuilder();
            BlockGetter.forEachBlockIntersectedBetween(from, to, target, (pos, step) -> {
                if (!hits.isEmpty()) hits.append(',');
                hits.append('[').append(pos.getX()).append(',').append(pos.getY()).append(',')
                    .append(pos.getZ()).append(',').append(step).append(']');
                return true;
            });
            System.out.printf(java.util.Locale.ROOT,
                "%s{\"from\":[%s,%s,%s],\"to\":[%s,%s,%s],\"min\":[%s,%s,%s],\"max\":[%s,%s,%s],\"hits\":[%s]}%n",
                i == 0 ? "" : ",", bits(x),bits(y),bits(z),bits(to.x),bits(to.y),bits(to.z),bits(target.minX),bits(target.minY),bits(target.minZ),
                bits(target.maxX),bits(target.maxY),bits(target.maxZ),hits);
        }
        System.out.println("]");
    }
}
