import java.util.*;
import net.minecraft.world.level.border.WorldBorder;
public class WorldBorderOracle {
    static String bits(double... xs) { StringJoiner j=new StringJoiner(",","[","]"); for(double x:xs) j.add(Long.toUnsignedString(Double.doubleToRawLongBits(x))); return j.toString(); }
    public static void main(String[] args) {
        var out=System.out;
        net.minecraft.SharedConstants.tryDetectVersion(); net.minecraft.server.Bootstrap.bootStrap();
        WorldBorder border=new WorldBorder(); border.applyInitialSettings(0);
        Random random=new Random(262);
        out.println("[");
        for(int i=0;i<180;i++) {
            int operation = i==0?0:random.nextInt(6);
            double a=(random.nextInt(200)+1)/4.0, b=(random.nextInt(40)+1);
            switch(operation) {
                case 0 -> border.setSize(a);
                case 1 -> border.lerpSizeBetween(border.getSize(),a,(long)b,0);
                case 2 -> border.tick();
                case 3 -> border.setCenter(a,b);
                case 4 -> { for(int n=0;n<(int)b;n++)border.tick(); }
                case 5 -> border.setAbsoluteMaxSize((int)b);
            }
            out.println((i==0?"":",")+"{\"op\":"+operation+",\"a\":"+a+",\"b\":"+b+",\"values\":"+bits(border.getSize(),border.getMinX(),border.getMinZ(),border.getMaxX(),border.getMaxZ())+",\"remaining\":"+border.getLerpTime()+"}");
        }
        out.println("]");
    }
}
