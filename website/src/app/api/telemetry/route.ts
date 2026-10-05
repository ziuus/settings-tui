import { NextResponse } from 'next/server';
import { db } from '@/lib/firebase';

export async function POST(request: Request) {
  try {
    const data = await request.json();
    
    // Add timestamp to telemetry event
    const event = {
      ...data,
      timestamp: new Date().toISOString(),
    };

    // Store the telemetry ping in Firestore under 'telemetry_pings' collection
    await db.collection('telemetry_pings').add(event);
    
    // Also update a daily aggregated stats document
    const today = new Date().toISOString().split('T')[0];
    const statsRef = db.collection('telemetry_stats').doc(today);
    
    // Atomically increment the total downloads / active users count for today
    // and keep track of unique machine IDs
    await db.runTransaction(async (t) => {
      const doc = await t.get(statsRef);
      if (!doc.exists) {
        t.set(statsRef, {
          total_pings: 1,
          unique_machines: [data.machine_id],
        });
      } else {
        const currentData = doc.data()!;
        const unique = new Set(currentData.unique_machines || []);
        unique.add(data.machine_id);
        
        t.update(statsRef, {
          total_pings: currentData.total_pings + 1,
          unique_machines: Array.from(unique),
        });
      }
    });

    return NextResponse.json({ success: true });
  } catch (error) {
    console.error("Telemetry Error:", error);
    return NextResponse.json({ error: 'Failed to record telemetry' }, { status: 500 });
  }
}
