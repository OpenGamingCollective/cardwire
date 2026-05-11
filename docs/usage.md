# Usage

## Querying GPUs

To have cardwire list all detected GPUs, use:

```
cardwire list
```

For each detected GPU, the command will return:
- An identifier (`ID`) (used for manual blocking and unblocking)
- The GPU's name (`NAME`)
- The GPU's PCI address (`PCI`) 
- The associated render node (`RENDER`)
- The associated device node (`CARD`) 
- Whether the GPU has been identified as the default GPU (`DEFAULT`). Default GPUs will remain available when cardwire is set to integrated.  
- Whether the GPU is currently blocked (`BLOCKED`) 

## Mode switching 

GPU modes can be switched using the `cardwire set` command. 

To have carwdire block all GPUs except the default GPU, use: 

```
cardwire set integrated
``` 

To have cardwire allow access to all GPUs, use 

```
cardwire set hybrid 
``` 

## Manual blocking and unblocking 


> [!IMPORTANT] 
> To prevent system breakage, cardwire will not block default GPUs, even when explicitly instructed to do so.

If more granular control over several GPUs is required, cardwire also allows manually blocking individual GPUs by ID. To do so, it needs to be set to manual mode: 

```
cardwire set manual 
``` 

Once set to manual, GPU states can then be set by ID; to find the correct ID, see [Querying GPUs](#Querying-GPUs). 

To block the GPU with ID `1`: 

```
cardwire gpu 1 --block
```

To unblock: 

``` 
cardwire gpu 1 --unblock
```
