// Actual EntityReference cache, removed-entity invalidation, UUID codec and lookup.
import java.util.*;
import net.minecraft.world.entity.EntityReference;
import net.minecraft.world.level.entity.UniquelyIdentifyable;
import net.minecraft.nbt.*;
public class EntityReferenceOracle {
    static class Target implements UniquelyIdentifyable {
        final UUID uuid;
        boolean removed;
        Target(UUID uuid) { this.uuid=uuid; }
        public UUID getUUID() { return uuid; }
        public boolean isRemoved() { return removed; }
    }
    public static void main(String[] args) {
        var out=System.out;
        UUID uuid=new UUID(0x123456789abcdef0L,0xfedcba9876543210L);
        var targets=new Target[]{new Target(uuid),new Target(uuid),new Target(new UUID(0x1000000000000001L,0x2000000000000002L))};
        EntityReference<Target> reference=EntityReference.of(uuid);
        int lookup=-1;Random random=new Random(262);out.println("[");
        for(int i=0;i<600;i++) {
            int op=i%15==0?0:random.nextInt(5),index=random.nextInt(3);
            switch(op) {
                case 0 -> reference=EntityReference.of(uuid);
                case 1 -> { reference=EntityReference.of(targets[index]); }
                case 2 -> targets[index].removed=!targets[index].removed;
                case 3 -> lookup=random.nextInt(4)-1;
            }
            int[] calls={0};final int selected=lookup;
            var hit=reference.getEntity(key->{calls[0]++;return selected<0?null:targets[selected].uuid.equals(key)?targets[selected]:null;},Target.class);
            int result=-1;for(int n=0;n<targets.length;n++)if(targets[n]==hit)result=n;
            var encoded=(IntArrayTag)EntityReference.<Target>codec().encodeStart(NbtOps.INSTANCE,reference).getOrThrow();
            var ints=encoded.getAsIntArray();
            out.println((i==0?"":",")+"["+op+","+index+","+lookup+",["+targets[0].removed+","+targets[1].removed+","+targets[2].removed+"],"+result+","+calls[0]+",["+ints[0]+","+ints[1]+","+ints[2]+","+ints[3]+"]]");
        }
        out.println("]");
    }
}
