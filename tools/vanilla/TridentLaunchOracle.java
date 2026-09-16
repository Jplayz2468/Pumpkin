// Numeric oracle for TridentItem.releaseUsing's launch expression, using real Mth.
// This does not invoke the item-use/world/network lifecycle.
import java.util.*;
import net.minecraft.util.Mth;
public class TridentLaunchOracle {
    public static void main(String[] args) {
        Random random=new Random(262);System.out.println("[");
        for(int i=0;i<1000;i++) {
            float yaw=(random.nextFloat()-.5F)*7200,pitch=(random.nextFloat()-.5F)*180,strength=random.nextFloat()*8;
            float x=-Mth.sin(yaw*(float)(Math.PI/180))*Mth.cos(pitch*(float)(Math.PI/180));
            float y=-Mth.sin(pitch*(float)(Math.PI/180));
            float z=Mth.cos(yaw*(float)(Math.PI/180))*Mth.cos(pitch*(float)(Math.PI/180));
            float dist=Mth.sqrt(x*x+y*y+z*z);
            x*=strength/dist;y*=strength/dist;z*=strength/dist;
            System.out.println((i==0?"":",")+"["+Integer.toUnsignedLong(Float.floatToRawIntBits(yaw))+","+Integer.toUnsignedLong(Float.floatToRawIntBits(pitch))+","+Integer.toUnsignedLong(Float.floatToRawIntBits(strength))+","+BlockClipOracle.bits(x,y,z)+"]");
        }
        System.out.println("]");
    }
}
